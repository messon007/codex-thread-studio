// Real public iframe navigation; geometry is observed by test-only init scripts.
import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { randomUUID } from 'node:crypto'
const directory = await mkdtemp(join(await realpath('target'),'table-navigation-'))
const binary = resolve(process.argv.slice(2).find(arg=>!arg.startsWith('--')) || 'target/debug/examples/embedded-host')
const text = '\ufeff# Public 中文😀\r\n\r\nParagraph needle.\r\n\r\n## Data\r\n\r\n| name | value |\r\n| --- | --- |\r\n' + Array.from({length:120},(_,n)=>`| row ${n} | ${n} |\r\n`).join('')+'\r\n'+'中😀'.repeat(6000)+'\r\n'
const token = randomUUID(), urls = [], children = [], caches = [], errors = []
let browser, report
const server = createServer(async (request,response) => {
  const file = {'/host-client.mjs':'web/host-client.mjs','/reader-bridge.mjs':'web/reader-bridge.mjs','/navigation-contract.mjs':'tests/browser/contracts/embedded-table-navigation.mjs'}[request.url]
  response.setHeader('Content-Type',file || request.url==='/contract.mjs'?'text/javascript':'text/html')
  if(file) response.end(await readFile(file))
  else if(request.url==='/contract.mjs') response.end(`import {tableNavigationContract} from '/navigation-contract.mjs';export const run=()=>tableNavigationContract(${JSON.stringify(urls)},${JSON.stringify(token)})`)
  else response.end('<!doctype html><html><body style="margin:0"></body></html>')
})
server.listen(0,'127.0.0.1');await once(server,'listening')
const origin = `http://127.0.0.1:${server.address().port}`
try {
  for (const writable of [false,true]) {
    const source = join(directory,writable?'editable.md':'readonly.md'), cache=join(directory,`cache-${Number(writable)}`)
    await writeFile(source,text);await mkdir(cache);caches.push({cache,source})
    const child=spawn(binary,[source,`--parent=${origin}`,...(writable?['--write']:[])],{env:{...process.env,READER_CACHE_DIR:cache},stdio:['pipe','pipe','pipe']});children.push(child)
    let stdout='',stderr='';child.stdout.on('data',data=>stdout+=data);child.stderr.on('data',data=>stderr+=data)
    const deadline=Date.now()+30000
    while(!stdout.includes('\n')&&child.exitCode==null&&Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,20))
    assert.match(stdout,/http:\/\/127\.0\.0\.1:/,stderr);urls.push(stdout.trim())
  }
  const init = (await readFile('tests/browser/table-navigation-probe.js','utf8')).replace('__MARKNORIA_TABLE_TEST__',JSON.stringify({origins:urls.map(url=>new URL(url).origin),parentOrigin:origin,token}))
  if(process.argv.includes('--webkit')) {
    const script = join(directory,'geometry-probe.js');await writeFile(script,init)
    const result = await promisify(execFile)('/usr/bin/python3',['tests/browser/contract-webkit.py',origin,'1600','1000','--init-script',script],{timeout:190000})
    report=JSON.parse(result.stdout);assert.equal(report.nativeDisplay,'GdkWaylandDisplay')
  } else {
    const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
    const page=await browser.newPage({viewport:{width:1600,height:1000}});await page.addInitScript({content:init})
    page.on('pageerror',error=>errors.push(String(error)))
    await page.goto(origin);report=await page.evaluate(async()=>await(await import('/contract.mjs')).run())
    assert.deepEqual(errors,[]);report.browserVersion=browser.version()
  }
  await writeFile(join(directory,'evidence.json'),JSON.stringify(report,null,2))
  const {evidence,...summary}=report
  console.log(JSON.stringify({ok:true,...summary,artifacts:directory}))
} catch(error) {
  await writeFile(join(directory,'failure.json'),JSON.stringify({message:error.message,stack:error.stack},null,2))
  process.stderr.write(String(error.stack)+'\n'); throw error
} finally {
  await browser?.close()
  server.closeAllConnections();server.close()
  for(const child of children) if(child.exitCode==null&&child.signalCode==null) {const exited=once(child,'exit');child.stdin.end('\n');await exited;assert.equal(child.exitCode,0)}
  for(const {source,cache} of caches) {
    assert.equal(await readFile(source,'utf8'),text)
    const deadline=Date.now()+5000
    while((await readdir(cache)).length && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,20))
    assert.deepEqual(await readdir(cache),[])
  }
}
