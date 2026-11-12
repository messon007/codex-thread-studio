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
const excludedSyntax=['cobol','d','hlsl','fsharp','ocaml','idris','odin','c-sharp','perl','ruby','elixir','vb','php']
const syntaxText=language=>`profile_${language} 中文 😀 <script>globalThis.syntaxExecuted=true</script> &`
const file=join(project,'document.md');await writeFile(file,'# Full shell sentinel\n\nA selectable paragraph.\n\n## Table\n\n| Name | Value |\n|---|---|\n| alpha | 42 |\n\n'+excludedSyntax.map(language=>'```'+language+'\n'+syntaxText(language)+'\n```\n').join('\n')+'\n```rust\nfn retained_rust() -> bool { true }\n```\n')
const largeTable=join(project,'oversized.md'),largeRow='| large | '+('中文😀&'.repeat(27000))+' OriginalTailNeedle |\r\n'
await writeFile(largeTable,'# Bounded table\r\n\r\n| Key | Value |\r\n| --- | --- |\r\n'+largeRow)
const csv=join(project,'wide.csv');await writeFile(csv,['ID,Count,Description,'+Array.from({length:20},(_,i)=>'Field'+i).join(','),...Array.from({length:123},(_,i)=>[i,i+1,'Long cell '+('safe content '.repeat(8)),...Array.from({length:20},(_,n)=>n+i)].join(','))].join('\n'))
const pdf=join(project,'document.pdf');await writeFile(pdf,pdfFixture())
const epub=JSON.parse(execFileSync('/usr/bin/python3',[join(root,'crates/marknoria/tests/browser/epub_fixtures.py'),project],{encoding:'utf8'})).book
const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),CODEX_THREAD_STUDIO_CODEX_BIN:'/intentionally-unavailable-codex',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache')}
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
 const cases=[['markdown',file],['large-table',largeTable],['table',csv],['pdf',pdf],['epub',epub]].filter(([format])=>!process.argv.includes('--markdown-only')||format==='markdown'), results=[]
 await writeFile(join(dir,'corpus.json'),JSON.stringify({project,cases}))
 const settings=async(theme,locale='en-US',reading=null)=>{
  await page.evaluate(()=>__studioDeveloper.click('#settings-button'))
  await page.locator('[data-settings-pane=general]').click()
  await page.locator('#theme-select').selectOption(theme)
  await page.locator('#language-select').selectOption(locale)
  if(reading){
   await page.locator('[data-settings-pane=typography]').click()
   await page.locator('#content-font-size').fill(String(reading.text))
   await page.locator('#code-font-size').fill(String(reading.code))
   await page.locator('#reader-highlight-matches').setChecked(reading.matches)
   await page.locator('#reader-automatic-code-size').setChecked(reading.auto)
  }
  await page.locator('#settings-form').evaluate(form=>form.requestSubmit())
  await page.locator('#settings-dialog').waitFor({state:'hidden'})
 }
 const command=(method,params={})=>page.evaluate(({method,params})=>shellClient.request(method,params),{method,params})
 const waitState=async predicate=>{
  const deadline=Date.now()+15000
  while(Date.now()<deadline){const state=await command('state');if(predicate(state))return state;await pause(25)}
  throw Error('Public reader state did not reach expected value')
 }
 // An old standalone preference must not override the host default.
 await page.evaluate(()=>localStorage.setItem('marknoria-highlight-selection-matches','true'))
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
  if(await page.locator('#artifact-search-match-case').isChecked() !== (format !== 'pdf'))throw Error('Host Match case default differs from the document format')
  if(await frame.locator('.reader-header').isVisible())throw Error('Child toolbar remains visible')
  for(const theme of ['light','dark']){
   await settings(theme)
   for(let i=0;i<100&&(await command('state')).presentation.theme!==theme;i++)await pause(20)
   if((await command('state')).presentation.theme!==theme)throw Error('Theme was not delivered')
   await inspect(await page.locator('.artifact-large-markdown-frame').elementHandle().then(node=>node.contentFrame()))
   await page.screenshot({path:join(dir,format+'-'+theme+'.png')})
  }
  if(format==='markdown'){
   for(const language of excludedSyntax){
    const code=frame.locator('pre code').filter({hasText:`profile_${language} 中文`})
    await code.waitFor({state:'attached'})
    if((await code.textContent()).trim()!==syntaxText(language))throw Error('Excluded syntax altered text: '+language)
    if(await code.locator('[class*=mn-h],script').count())throw Error('Excluded syntax used another grammar or unsafe HTML: '+language)
   }
   if(await frame.locator('pre code').filter({hasText:'fn retained_rust()'}).locator('[class*=mn-h]').count()===0)throw Error('Retained Rust grammar lost highlighting')
   if(await frame.locator('body').evaluate(()=>Boolean(globalThis.syntaxExecuted)))throw Error('Code fixture executed markup')

   if(first.presentation.highlightSelectionMatches!==false)throw Error('Standalone word preference overrode host default')
   await settings('dark','zh-CN',{text:32,code:11,matches:true,auto:false})
   await waitState(s=>s.presentation.contentFontSize===32&&s.presentation.codeFontSize===11&&s.presentation.highlightSelectionMatches)
   const fonts=await frame.locator('#space').evaluate(node=>({text:getComputedStyle(node.querySelector('td')).fontSize,code:getComputedStyle(node.querySelector('pre')).fontSize}))
   if(fonts.text!=='32px'||fonts.code!=='11px')throw Error('Text/code size roles were not applied: '+JSON.stringify(fonts))
   await settings('dark','en-US',{text:11,code:32,matches:true,auto:true})
   await waitState(s=>s.presentation.contentFontSize===11&&s.presentation.codeFontSize===null)
   // Open Source through real host controls, then invoke the child's shared keyboard dialog.
   await page.locator('#artifact-source').click()
   await waitState(s=>s.view==='source')
   await frame.locator('.cm-content').click()
   await page.keyboard.press('Control+g')
   await frame.locator('.reader-go-to-line[open]').waitFor()
   await frame.locator('#reader-go-to-line-input').fill('3')
   await frame.locator('.reader-go-to-line form').evaluate(form=>form.requestSubmit())
   await frame.locator('.reader-go-to-line').waitFor({state:'hidden'})
   const navigated=await waitState(s=>s.view==='source'&&s.position?.location?.line===3)
   if(navigated.capabilities.edit!==first.capabilities.edit||navigated.dirty)throw Error('Source navigation changed authority or dirtied the document')
   await page.locator('#artifact-preview').click()
   await waitState(s=>s.view==='preview')
   // Only booleans change: this must update immediately without a CSS mutation.
   await settings('dark','en-US',{text:11,code:32,matches:false,auto:false})
   await waitState(s=>s.presentation.codeFontSize===32&&!s.presentation.highlightSelectionMatches)
   await settings('dark','en-US',{text:15,code:14,matches:true,auto:true})
   await waitState(s=>s.presentation.codeFontSize===null&&s.presentation.highlightSelectionMatches)

   if((await command('state')).presentation.notes!=='host')throw Error('Reader Notes ownership was not delegated')
   if((await command('state')).presentation.selectionMenu!=='host')throw Error('Reader selection ownership was not delegated')
   if(await page.locator('#artifact-notes-tools button').count())throw Error('Unexpected document Favorites/Notes entry')
   if(await frame.locator('.notes-open,.notes-selection').count())throw Error('Unexpected child Notes controls')
   await page.evaluate(async()=>{const {setLanguage}=await import('/i18n.mjs');setLanguage('zh-CN')})
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
  if(format==='large-table'){
   await frame.locator('td[data-cell-preview="true"]').first().click()
   if(await frame.locator('.table-cell-copy').first().textContent()!=='Copy preview')throw Error('Abbreviated value claimed complete copy')
   await frame.locator('.table-cell-view').first().click()
   const dialog=frame.getByRole('dialog',{name:'Original source row',exact:true}),parts=[]
   await dialog.waitFor()
   for(let index=0;index<8;index++){
    await dialog.locator('pre:not([aria-busy])').waitFor()
    const part=await dialog.locator('pre').textContent()
    if(Buffer.byteLength(part)>128*1024||await dialog.locator('pre').count()!==1)throw Error('Original source window is unbounded')
    parts.push(part)
    if(await dialog.locator('.table-source-next').isDisabled())break
    await dialog.locator('.table-source-next').click()
   }
   if(parts.length<2||parts.join('')!==largeRow)throw Error('Original row lost bytes across bounded windows')
   if(await command('selection')!==null)throw Error('Raw source window published a rendered-cell comment anchor')
   await page.evaluate(async()=>{const {setLanguage}=await import('/i18n.mjs');setLanguage('zh-CN')})
   await frame.getByRole('dialog',{name:'原始源码行',exact:true}).waitFor()
   await frame.getByRole('button',{name:'复制本页源码',exact:true}).waitFor()
   await page.evaluate(async()=>{const {setLanguage}=await import('/i18n.mjs');setLanguage('en-US')})
   await dialog.getByRole('button',{name:'Close',exact:true}).click()
   if(await readFile(largeTable,'utf8')!=='# Bounded table\r\n\r\n| Key | Value |\r\n| --- | --- |\r\n'+largeRow)throw Error('Original fixture changed')
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
  const initial=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');const frame=document.querySelector('.artifact-large-markdown-frame');const client=createReaderClient(frame);await client.ready();const state=await client.request('state');if(state.presentation.highlightSelectionMatches!==true||state.presentation.codeFontSize!==null)throw Error('Reader preferences lost on reopen');client.disconnect();return {...state.timings,mounted:frame.contentDocument.querySelectorAll('#space *').length}})
  results.push({format,phase:'reopen-browser-cache',initial:{...initial,hostFirstUsefulMs:performance.now()-started}})
  await page.locator('#close-artifact').click()
 }
 if(errors.length)throw Error(JSON.stringify(errors))
 let webkit
 if(process.argv.includes('--webkit'))webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-shell-webkit.py'),url,JSON.stringify({project,cases:cases.filter(([format])=>format!=='large-table'),output:dir,profileInteractions:process.argv.includes('--profile-interactions')})],{timeout:175000,maxBuffer:1000000})).stdout)
 if(process.argv.includes('--webkit')&&!webkit?.ok)throw Error('Native WebKit shell did not pass')
 console.log(JSON.stringify({ok:true,fullApp:true,noAppReplacement:true,excludedSyntaxPlaintext:excludedSyntax,retainedRustHighlight:true,liveLocale:true,hostMenuOwnership:true,railSearch:true,readingControls:true,sourceGoToLine:true,boundedOriginalRow:true,results,webkit,artifacts:dir}))

}catch(error){console.error('Artifacts: '+dir);throw error}finally{await browser?.close();if(server.exitCode==null){server.kill('SIGTERM');await Promise.race([once(server,'exit'),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')}}
