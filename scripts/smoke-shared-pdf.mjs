// Runs the Studio adapter served by the actual gateway, using only synthetic data.
import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { pdfFixture } from '../crates/marknoria/tests/browser/pdf-fixture.mjs'
const root=resolve(new URL('../',import.meta.url).pathname)
const parent=resolve(process.env.STUDIO_SMOKE_TMP || join(root,'target/shared-pdf-smoke'))
await mkdir(parent,{recursive:true})
const directory=await mkdtemp(join(parent,'run-'))
const fixture=join(directory,'fixture.pdf')
await writeFile(fixture,pdfFixture())
const environment={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',XDG_CONFIG_HOME:join(directory,'config'),XDG_DATA_HOME:join(directory,'data'),XDG_CACHE_HOME:join(directory,'cache'),READER_CACHE_DIR:join(directory,'reader-cache')}
for(const name of ['XDG_RUNTIME_DIR','WAYLAND_DISPLAY','GDK_BACKEND','DBUS_SESSION_BUS_ADDRESS','WEBKIT_DISABLE_COMPOSITING_MODE'])if(process.env[name])environment[name]=process.env[name]
const child=spawn(process.env.STUDIO_REMOTE_BINARY || join(root,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env:environment,stdio:['ignore','pipe','pipe']})
let output='',browser,server
child.stdout.on('data',bytes=>{output=(output+bytes).slice(-100000)})
child.stderr.on('data',()=>{})
try{
 const deadline=Date.now()+30000
 while(!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(output)){
  if(Date.now()>deadline || child.exitCode!=null)throw Error('Synthetic Studio gateway did not start')
  await new Promise(resolve=>setTimeout(resolve,50))
 }
 const gateway=new URL(output.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0])
 const token=new URLSearchParams(gateway.hash.slice(1)).get('token')
 const original=await fetch(new URL('/studio/review-pdf',gateway),{method:'POST',headers:{Authorization:'Bearer '+token,'Content-Type':'application/json'},body:JSON.stringify({root:directory,path:fixture})})
 assert.equal(original.status,200)
 const bytes=Buffer.from(await original.arrayBuffer())
 assert.deepEqual(bytes,pdfFixture())
 assert.equal((await fetch(new URL('/studio/review-pdf',gateway),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({root:directory,path:fixture})})).status,401)
 // This ordinary host supplies only already-authorized bytes and static modules.
 // No gateway credentials enter the component, worker, PDF or test page.
 server=createServer(async(request,response)=>{
  response.setHeader('Content-Security-Policy',"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; connect-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data: blob:; object-src 'none'")
  if(request.url==='/'){response.setHeader('Content-Type','text/html');response.end('<!doctype html><html><body></body></html>');return}
  if(request.url==='/fixture.pdf'){response.setHeader('Content-Type','application/pdf');response.end(bytes);return}
  if(request.url==='/contract.mjs'){response.setHeader('Content-Type','text/javascript');response.end(await readFile(join(root,'scripts/smoke-shared-pdf-contract.mjs')));return}
  if(request.url!=='/pdf-reader.mjs' && !/^\/reader-assets\/[a-zA-Z0-9_./-]+$/u.test(request.url)){response.writeHead(404);response.end();return}
  if(request.url.includes('..')){response.writeHead(404);response.end();return}
  const upstream=await fetch(new URL(request.url,gateway))
  response.statusCode=upstream.status
  response.setHeader('Content-Type',upstream.headers.get('content-type') || 'application/octet-stream')
  response.end(Buffer.from(await upstream.arrayBuffer()))
 })
 server.listen(0,'127.0.0.1');await once(server,'listening')
 const url=`http://127.0.0.1:${server.address().port}/`
 let result
 if(process.argv.includes('--webkit')){
  result=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'crates/marknoria/tests/browser/contract-webkit.py'),url],{env:{...environment,PATH:'/usr/bin:/bin'},timeout:190000,maxBuffer:1000000})).stdout)
 }else{
  const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const page=await browser.newPage({viewport:{width:1100,height:800}}),errors=[]
  page.on('pageerror',error=>errors.push(error.message))
  await page.goto(url)
  result=await page.evaluate(async()=>(await import('/contract.mjs')).run())
  assert.deepEqual(errors,[])
  result.browserVersion=browser.version()
 }
 assert.equal(result.sharedPdf,true)
 assert.equal(result.assertions,12)
 console.log(JSON.stringify({ok:true,gatewayGrant:true,noGatewayCredentialInRenderer:true,...result}))
}finally{
 await browser?.close()
 if(server)await new Promise(resolve=>server.close(resolve))
 if(child.exitCode==null){child.kill('SIGTERM');await Promise.race([once(child,'exit'),new Promise(resolve=>{const timer=setTimeout(resolve,3000);timer.unref()})]);if(child.exitCode==null)child.kill('SIGKILL')}
}
