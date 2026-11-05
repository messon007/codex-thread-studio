// Actual unmodified Studio shell; uses its existing developer opener and public reader client.
import {spawn} from 'node:child_process'
import {mkdtemp,mkdir,writeFile,readFile} from 'node:fs/promises'
import {join,resolve} from 'node:path'
import {once} from 'node:events'
import {fileURLToPath} from 'node:url'
import {execFileSync,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {pdfFixture} from '../crates/marknoria/tests/browser/pdf-fixture.mjs'
const root=fileURLToPath(new URL('../',import.meta.url)), output=join(root,'target/reader-shell-smoke')
await mkdir(output,{recursive:true})
const dir=await mkdtemp(join(output,'run-')),project=join(dir,'project')
await mkdir(project)
const file=join(project,'document.md');await writeFile(file,'# Full shell sentinel\n\nA selectable paragraph.\n\n## Table\n\n| Name | Value |\n|---|---|\n| alpha | 42 |\n')
const csv=join(project,'wide.csv');await writeFile(csv,['ID,Count,Description,'+Array.from({length:20},(_,i)=>'Field'+i).join(','),...Array.from({length:123},(_,i)=>[i,i+1,'Long cell '+('safe content '.repeat(8)),...Array.from({length:20},(_,n)=>n+i)].join(','))].join('\n'))
const pdf=join(project,'document.pdf');await writeFile(pdf,pdfFixture())
const epub=JSON.parse(execFileSync('/usr/bin/python3',[join(root,'crates/marknoria/tests/browser/epub_fixtures.py'),project],{encoding:'utf8'})).book
const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache')}
const server=spawn(process.env.STUDIO_REMOTE_BINARY||join(root,'target/debug/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env,stdio:['ignore','pipe','pipe']})
let logs='',browser
server.stdout.on('data',x=>logs+=x);server.stderr.on('data',()=>{})
const pause=x=>new Promise(r=>setTimeout(r,x))
try{
 const deadline=Date.now()+30000
 while(!/http:\/\/127.0.0.1:\d+\/#token=[a-f0-9]{32}/.test(logs)){if(Date.now()>deadline)throw Error('Startup timeout');await pause(30)}
 const url=logs.match(/http:\/\/127.0.0.1:\d+\/#token=[a-f0-9]{32}/)[0]
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE)
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:1000}}),errors=[]
 page.on('pageerror',e=>errors.push(String(e)))
 await page.goto(url)
 await page.waitForFunction(()=>Boolean(window.__studioDeveloper))
 const actual=await page.request.get(new URL('/document-workspace-controller.mjs',url).href)
 if(await actual.text()!==await readFile(join(root,'ui/document-workspace-controller.mjs'),'utf8'))throw Error('Stale Studio binary frontend')
 const cases=[['markdown',file],['table',csv],['pdf',pdf],['epub',epub]], results=[]
 await writeFile(join(dir,'corpus.json'),JSON.stringify({project,cases}))
 const settings=async(theme,locale='en-US')=>{
  await page.evaluate(()=>__studioDeveloper.click('#settings-button'))
  await page.locator('#theme-select').selectOption(theme)
  await page.locator('#language-select').selectOption(locale)
  await page.locator('#settings-form').evaluate(form=>form.requestSubmit())
  await page.locator('#settings-dialog').waitFor({state:'hidden'})
 }
 const command=(method,params={})=>page.evaluate(({method,params})=>shellClient.request(method,params),{method,params})
 const inspect=async frame=>{
  const check=()=>{
   const visible=node=>node.getClientRects().length&&getComputedStyle(node).visibility!=='hidden'
   const unnamed=[...document.querySelectorAll('button,input:not([type=hidden]),select,textarea')].filter(node=>visible(node)&&!node.disabled&&!node.getAttribute('aria-label')&&!node.getAttribute('title')&&!node.textContent.trim()&&!node.labels?.length&&!node.getAttribute('placeholder')).map(node=>node.outerHTML.slice(0,180))
   const clipped=[...document.querySelectorAll('dialog[open],#reader-outline:not([hidden])')].filter(visible).filter(node=>{const r=node.getBoundingClientRect();return r.x<0||r.y<0||r.right>innerWidth+1||r.bottom>innerHeight+1}).map(node=>node.className||node.id)
   return {unnamed,clipped}
  }
  const result=await frame.evaluate(check)
  if(result.unnamed.length||result.clipped.length)throw Error(JSON.stringify(result))
 }
 for(const [format,path] of cases){
  const started=performance.now()
  await page.evaluate(()=>window.shellClient?.disconnect())
  await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
  await page.locator('.artifact-large-markdown-frame').waitFor()
  await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled)
  const frame=page.frameLocator('.artifact-large-markdown-frame')
  await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.shellClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await shellClient.ready()})
  const first=await command('state');const initial={...first.timings,hostFirstUsefulMs:performance.now()-started,mounted:await frame.locator('#space *').count()}
  if(first.presentation.toolbar!=='host')throw Error('Duplicate child toolbar')
  if(await frame.locator('.reader-header').isVisible())throw Error('Child toolbar remains visible')
  for(const theme of ['light','dark']){
   await settings(theme)
   for(let i=0;i<100&&(await command('state')).presentation.theme!==theme;i++)await pause(20)
   if((await command('state')).presentation.theme!==theme)throw Error('Theme was not delivered')
   await inspect(await page.locator('.artifact-large-markdown-frame').elementHandle().then(node=>node.contentFrame()))
   await page.screenshot({path:join(dir,format+'-'+theme+'.png')})
  }
  if(format==='markdown'){
   const owner=await command('state'), window=(await command('sourceWindow',{line:1})).result
   const source=await command('sourceSelection',{start:window.start,end:window.end,fromUtf16:2,toUtf16:6})
   await command('notes',{request:{operation:'add',kind:'comment',revision:owner.revision,quote:source.quote,comment:'Close',anchor:{location:source.location}}})
   await page.locator('#artifact-notes-tools').getByRole('button',{name:'Notes',exact:true}).click()
   await page.locator('.notes-dialog').waitFor()
   await page.locator('.note-comment').filter({hasText:'Close'}).waitFor()
   await page.evaluate(async()=>{const {setLanguage}=await import('/i18n.mjs');setLanguage('zh-CN')})
   const commentsLabel=await page.evaluate(async()=>(await import('/i18n.mjs')).t('Comments'))
   await page.locator('.notes-filters').getByRole('button',{name:commentsLabel,exact:true}).waitFor()
   if(await page.locator('.note-comment').textContent()!=='Close')throw Error('Locale translated literal note data')
   await page.screenshot({path:join(dir,'notes-chinese.png')})
   await page.keyboard.press('Escape')
   if(!await page.locator('#artifact-rail').isVisible())throw Error('Modal Escape closed the document')
   await page.locator('#artifact-outline-toggle').click()
   await frame.getByRole('searchbox',{name:'筛选目录',exact:true}).waitFor()
   await page.evaluate(async()=>{const {setLanguage}=await import('/i18n.mjs');setLanguage('en-US')})
   await frame.getByRole('searchbox',{name:'Filter contents',exact:true}).waitFor()
   await frame.locator('#contents-close').click()
   await page.locator('#artifact-preview').focus()
   await page.keyboard.press('Control+f')
   if(!await page.locator('#artifact-search-input').isVisible())throw Error('Rail search shortcut unavailable')
   await page.keyboard.press('Escape')
  }
  if(format==='table'){
   if((await command('tableInfo',{table:0})).rows!==123)throw Error('Malformed synthetic table')
   await frame.locator('.table-query-open').first().click()
   await frame.locator('.table-query-dialog').waitFor()
   await page.setViewportSize({width:700,height:600})
   await inspect(await page.locator('.artifact-large-markdown-frame').elementHandle().then(node=>node.contentFrame()))
   await page.screenshot({path:join(dir,'table-sql-narrow.png')})
   await frame.locator('.table-query-dialog').getByRole('button',{name:'Close',exact:true}).click()
   await page.setViewportSize({width:1500,height:1000})
  }
  results.push({format,phase:'first-open-private-cache',initial,afterInteractions:(await command('state')).timings,themeSwitch:true,accessibleNames:true,containedDialogs:true})
 }
 await page.evaluate(()=>shellClient.disconnect())
 await page.locator('#close-artifact').click()
 for(const [format,path] of cases){
  const started=performance.now()
  await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
  await page.waitForFunction(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled)
  const initial=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');const frame=document.querySelector('.artifact-large-markdown-frame');const client=createReaderClient(frame);await client.ready();const state=await client.request('state');client.disconnect();return {...state.timings,mounted:frame.contentDocument.querySelectorAll('#space *').length}})
  results.push({format,phase:'reopen-browser-cache',initial:{...initial,hostFirstUsefulMs:performance.now()-started}})
  await page.locator('#close-artifact').click()
 }
 if(errors.length)throw Error(JSON.stringify(errors))
 let webkit
 if(process.argv.includes('--webkit'))webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-shell-webkit.py'),url,JSON.stringify({project,cases,output:dir})],{timeout:175000,maxBuffer:1000000})).stdout)
 if(process.argv.includes('--webkit')&&!webkit?.ok)throw Error('Native WebKit shell did not pass')
 console.log(JSON.stringify({ok:true,fullApp:true,noAppReplacement:true,liveLocale:true,modalEscape:true,railSearch:true,results,webkit,artifacts:dir}))

}catch(error){console.error('Artifacts: '+dir);throw error}finally{await browser?.close();if(server.exitCode==null){server.kill('SIGTERM');await Promise.race([once(server,'exit'),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')}}
