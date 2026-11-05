// Same generated corpus as smoke-reader-shell; diagnostics below are test-only.
import assert from 'node:assert/strict'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {once} from 'node:events'
import {readFile,writeFile,mkdir} from 'node:fs/promises'
import {resolve,join,basename} from 'node:path'
const corpus=resolve(process.argv[2]), cfg=JSON.parse(await readFile(join(corpus,'corpus.json'),'utf8'))
const out=join(corpus,'standalone');await mkdir(out,{recursive:true})
const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',READER_CACHE_DIR:join(out,'cache'),MARKNORIA_CONFIG_DIR:join(out,'config')}
const server=spawn(process.env.MARKNORIA_SERVER,[cfg.cases[0][1],'--root',cfg.project],{env,stdio:['ignore','pipe','pipe']})
let logs='',browser;server.stdout.on('data',v=>logs+=v);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(r=>setTimeout(r,ms))
async function inspectStandalone({format,name}) {
 const pause=()=>new Promise(r=>setTimeout(r,20)), end=performance.now()+30000
 while(window.readerExperiment?.index.name!==name||readerExperiment.metrics.firstContentMs==null){if(performance.now()>end)throw Error('Standalone readiness timeout: '+document.querySelector('#status').textContent);await pause()}
 const {index,metrics}=readerExperiment
 const sample=()=>({indexMs:index.indexMs,indexPeakRssKiB:index.peakRssKib,firstUsefulContentMs:metrics.firstContentMs,maxHeartbeatGapMs:metrics.maxHeartbeatGapMs,mounted:document.querySelectorAll('#space *').length,observedAt:Date.now()})
 const initial=sample()
 if(getComputedStyle(document.querySelector('.reader-header')).display==='none')throw Error('Standalone toolbar hidden')
 for(const theme of ['light','dark']){
  dispatchEvent(new CustomEvent('marknoria:theme-request',{detail:{theme}}))
  if(document.documentElement.dataset.theme!==theme)throw Error('Standalone theme did not switch')
 }
 if(format==='markdown'){
  document.querySelector('.notes-open').click()
  if(!document.querySelector('.notes-dialog[open]'))throw Error('Standalone Notes did not open')
  document.querySelector('.notes-close').click()
  document.querySelector('#contents-toggle').click()
  if(document.querySelector('#reader-outline').hidden)throw Error('Standalone Contents did not open')
  document.querySelector('#contents-close').click()
  document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'f',ctrlKey:true,bubbles:true,cancelable:true}))
  if(document.querySelector('#search').hidden)throw Error('Standalone search shortcut unavailable')
  document.querySelector('#search-close').click()
 }
 if(format==='table'){
  document.querySelector('.table-query-open').click()
  const dialog=document.querySelector('.table-query-dialog[open]');if(!dialog)throw Error('Standalone SQL did not open')
  const r=dialog.getBoundingClientRect();if(r.x<0||r.y<0||r.right>innerWidth+1||r.bottom>innerHeight+1)throw Error('Standalone SQL clipped')
  dialog.querySelector('[aria-label=Close]').click()
 }
 const visible=node=>node.getClientRects().length&&getComputedStyle(node).visibility!=='hidden'
 const unnamed=[...document.querySelectorAll('button,input:not([type=hidden]),select,textarea')].filter(node=>visible(node)&&!node.disabled&&!node.getAttribute('aria-label')&&!node.getAttribute('title')&&!node.textContent.trim()&&!node.labels?.length&&!node.getAttribute('placeholder'))
 if(unnamed.length)throw Error('Unnamed standalone control: '+unnamed[0].outerHTML.slice(0,200))
 await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))
 return {format,initial,afterInteractions:sample(),theme:true,accessibleNames:true}
}
try {
 while(!logs.includes('http://')){if(server.exitCode!=null)throw Error('Standalone startup failed');await pause(20)}
 const url=new URL(logs.trim().split('\n')[0]), token=url.hash.slice(1)
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE)
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:1000}}), errors=[], results=[]
 page.on('pageerror',e=>errors.push(String(e)))
 for(const phase of ['fresh-application-cache','reopen-browser-cache'])for(const [format,path] of cfg.cases){
  const started=Date.now()
  const response=await fetch(new URL('/api/open?path='+encodeURIComponent(basename(path)),url),{method:'POST',headers:{'x-reader-token':token}})
  assert.equal(response.status,200,await response.text())
  if(page.url()===url.href)await page.reload();else await page.goto(url.href)
  const sample=await page.evaluate(inspectStandalone,{format,name:basename(path)})
  sample.hostFirstUsefulMs=sample.initial.observedAt-started;sample.phase=phase
  assert.ok(sample.initial.mounted<5000,'Unbounded standalone DOM')
  if(phase==='fresh-application-cache')await page.screenshot({path:join(out,format+'-dark.png')})
  results.push(sample)
 }
 assert.deepEqual(errors,[])
 await browser.close();browser=null
 const script=join(out,'native-inspect.js');await writeFile(script,'('+inspectStandalone.toString()+')')
 let webkit
 if(process.argv.includes('--webkit'))webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[new URL('./smoke-standalone-reader-webkit.py',import.meta.url).pathname,url.href,JSON.stringify({...cfg,output:out,script})],{timeout:180000,maxBuffer:1000000})).stdout)
 if(process.argv.includes('--webkit'))assert.equal(webkit.ok,true)
 console.log(JSON.stringify({ok:true,sharedCorpus:corpus,results,webkit,artifacts:out}))
} finally {await browser?.close();if(server.exitCode==null){server.kill('SIGTERM');await Promise.race([once(server,'exit'),pause(3000)])}}
