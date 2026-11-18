import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { randomUUID } from 'node:crypto'
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const args=process.argv.slice(2), binaries=args.filter(value=>!value.startsWith('--'))
const base=resolve('target/reader-layout-checks');await mkdir(base,{recursive:true})
const directory=await mkdtemp(join(base,'run-')), token=randomUUID(), urls=[], children=[], caches=[]
const section=n=>'# Layout regression '+n+'\n\n'+('A paragraph for reading layout. '.repeat(20)+'\n\n').repeat(3)+
  '| Topic | Kind | Question |\n| --- | --- | --- |\n'+Array.from({length:14},(_,i)=>`| Topic ${i} | Language | Example question ${i} |`).join('\n')+
  '\n\n'+('Text after the table must not overlap it. '.repeat(12)+'\n\n').repeat(5)
const text=Array.from({length:8},(_,n)=>section(n)).join('\n'), source=join(directory,'layout.md')
await writeFile(source,text)
let browser, page
const parent=createServer(async(request,response)=>{
  const file={'/host-client.mjs':'web/host-client.mjs','/reader-bridge.mjs':'web/reader-bridge.mjs',
    '/layout-contract.mjs':'tests/browser/contracts/reader-layout.mjs','/embedded-layout-contract.mjs':'tests/browser/contracts/embedded-reader-layout.mjs'}[request.url]
  response.setHeader('Content-Type',file||request.url==='/contract.mjs'?'text/javascript':'text/html')
  if(file) response.end(await readFile(file))
  else if(request.url==='/contract.mjs') response.end(`import {embeddedLayoutContract} from '/embedded-layout-contract.mjs';export const run=()=>embeddedLayoutContract(${JSON.stringify(urls)},${JSON.stringify(token)})`)
  else response.end('<!doctype html><html><body style="margin:0"></body></html>')
})
parent.listen(0,'127.0.0.1');await once(parent,'listening')
const origin=`http://127.0.0.1:${parent.address().port}`
async function launch(binary,argv,kind){
  const cache=join(directory,`${kind}-cache`);await mkdir(cache);caches.push(cache)
  const child=spawn(resolve(binary),argv,{env:{...process.env,READER_CACHE_DIR:cache,MARKNORIA_CONFIG_DIR:join(directory,`${kind}-config`)},stdio:['pipe','pipe','pipe']})
  children.push({child,kind});let out='',err=''
  child.stdout.on('data',value=>out+=value);child.stderr.on('data',value=>err+=value)
  const end=Date.now()+30000
  while(!out.includes('\n') && child.exitCode==null && Date.now()<end) await new Promise(resolve=>setTimeout(resolve,20))
  assert.match(out,/http:\/\/127\.0\.0\.1:/,err)
  return out.trim().split('\n')[0]
}
try {
  const standalone=await launch(binaries[0]||'target/release/marknoria-server',[source,'--root',directory],'standalone')
  for(const writable of [false,true]) urls.push(await launch(binaries[1]||'target/release/examples/embedded-host',[source,`--parent=${origin}`,...(writable?['--write']:[])],`embedded-${writable}`))
  const probe=(await readFile('tests/browser/reader-layout-probe.js','utf8')).replace('__MARKNORIA_LAYOUT_TEST__',JSON.stringify({origins:[standalone,...urls].map(url=>new URL(url).origin),parentOrigin:origin,token,font:(await readFile('node_modules/katex/dist/fonts/KaTeX_Typewriter-Regular.woff2')).toString('base64')}))
  const contract=(await readFile('tests/browser/contracts/reader-layout.mjs','utf8')).replaceAll('export ','')
  let normal,embedded,browserVersion
  if(args.includes('--webkit')){
    const init=join(directory,'probe.js'),script=join(directory,'standalone-contract.js');await writeFile(init,probe);await writeFile(script,contract)
    const run=async(url,extra=[])=>JSON.parse((await promisify(execFile)('/usr/bin/python3',['tests/browser/contract-webkit.py',url,'1440','950','--init-script',init,...extra],{env:{...process.env,GDK_BACKEND:'wayland',XDG_CACHE_HOME:join(directory,'webkit-cache')},timeout:190000,maxBuffer:16*1024*1024})).stdout)
    normal=await run(standalone,['--run-script',script]);embedded=await run(origin)
    assert.equal(normal.nativeDisplay,'GdkWaylandDisplay');assert.equal(embedded.nativeDisplay,'GdkWaylandDisplay');browserVersion=embedded.browserVersion
  }else{
    const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
    browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
    page=await browser.newPage({viewport:{width:1440,height:950}});const errors=[]
    page.on('pageerror',error=>errors.push(String(error)));await page.addInitScript({content:probe})
    await page.goto(standalone);normal=await page.evaluate(`(async()=>{${contract};return run()})()`)
    await page.screenshot({path:join(directory,'standalone-draft.png')})
    await page.goto(origin);embedded=await page.evaluate(async()=>await(await import('/contract.mjs')).run())
    assert.deepEqual(errors,[]);browserVersion=browser.version()
  }
  await writeFile(join(directory,'report.json'),JSON.stringify({ok:true,browserVersion,normal,embedded},null,2))
  console.log(JSON.stringify({ok:true,browserVersion,standaloneAssertions:normal.assertions,embeddedAssertions:embedded.assertions,artifacts:directory}))
}catch(error){if(page) await writeFile(join(directory,'failed-evidence.json'),JSON.stringify(await page.evaluate(()=>window.readerLayoutEvidence).catch(()=>null),null,2));await writeFile(join(directory,'failure.json'),JSON.stringify({error:String(error),stack:error.stack},null,2));console.error('Layout artifacts:',directory);throw error}
finally{
  await browser?.close();parent.closeAllConnections();parent.close()
  for(const {child,kind} of children) if(child.exitCode==null&&child.signalCode==null){const exited=once(child,'exit');kind==='standalone'?child.kill('SIGINT'):child.stdin.end('\n');await exited;assert.equal(child.exitCode,0)}
  assert.equal(await readFile(source,'utf8'),text)
  for(const cache of caches) assert.deepEqual(await readdir(cache),[])
}
