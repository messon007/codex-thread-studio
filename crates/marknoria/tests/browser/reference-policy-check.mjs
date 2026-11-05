// Changing resource grants must not dispose the document or its editor draft.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, readdir, realpath, symlink, unlink, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const directory = await mkdtemp(join(await realpath('target'), 'reference-policy-'))
const project = join(directory,'project'), shared = join(directory,'shared'), cache = join(directory,'cache')
for (const path of [project,shared,cache]) await mkdir(path)
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=','base64')
await writeFile(join(project,'project.png'),png)
await writeFile(join(shared,'first.png'),png)
await writeFile(join(shared,'second.png'),Buffer.concat([png,Buffer.from('second snapshot')]))
const alias = join(project,'alias.png')
await symlink(join(shared,'first.png'),alias)
const source = join(project,'policy.md'), policy = join(directory,'roots.json')
await writeFile(source,'# Policy\n\n![project](project.png) ![shared](../shared/first.png) ![alias](alias.png)\n')
await writeFile(policy,'[]')
const host = createServer(async (request,response) => {
  if (['/host-client.mjs','/reader-bridge.mjs'].includes(request.url)) {
    response.setHeader('Content-Type','text/javascript');response.end(await readFile('web'+request.url))
  } else { response.setHeader('Content-Type','text/html');response.end('<!doctype html><html><body></body></html>') }
})
host.listen(0,'127.0.0.1'); await once(host,'listening')
const origin = `http://127.0.0.1:${host.address().port}`
const child = spawn(resolve(process.env.MARKNORIA_EMBEDDED_BINARY || 'target/debug/examples/embedded-host'),
  [source,'--write',`--parent=${origin}`,`--reference-root=${project}`,`--reference-roots-file=${policy}`],
  {env:{...process.env,READER_CACHE_DIR:cache},stdio:['pipe','pipe','pipe']})
let stdout='',stderr='',browser
child.stdout.on('data',bytes=>stdout+=bytes);child.stderr.on('data',bytes=>stderr+=bytes)
const pause = ms=>new Promise(resolve=>setTimeout(resolve,ms))
try {
  const deadline=Date.now()+30000
  while(!stdout.includes('\n')&&child.exitCode==null&&Date.now()<deadline) await pause(20)
  assert.match(stdout,/http:\/\/127\.0\.0\.1:/,stderr)
  const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const page=await browser.newPage({viewport:{width:1300,height:900}}),errors=[]
  page.on('pageerror',error=>errors.push(String(error)))
  await page.goto(origin)
  await page.evaluate(async url=>{
    const {createReaderClient}=await import('/host-client.mjs')
    const frame=document.createElement('iframe');frame.title='Editable policy document';frame.src=url
    frame.style.cssText='width:1100px;height:800px';document.body.append(frame)
    window.policyClient=createReaderClient(frame);await policyClient.ready()
  },stdout.trim())
  const frame=page.frames().find(frame=>frame.url()===stdout.trim())
  const request=(method,params={})=>page.evaluate(({method,params})=>policyClient.request(method,params),{method,params})
  const ready=async id=>{
    const deadline=Date.now()+30000
    while(Date.now()<deadline) {
      try { const value=await request('referenceImageInfo',{id});if(value.state==='ready')return value }
      catch(error) { if(!/busy|budget|preparing/i.test(error.message))throw error }
      await pause(30)
    }
    throw Error('Reference was not prepared')
  }
  const denied=async(method,params)=>{
    const result=await request(method,params).then(()=>null,error=>error.message)
    assert.match(result||'',/\(403\)/)
  }
  await ready(0)
  await denied('referenceImageInfo',{id:1})
  await request('setView',{view:'edit'})
  const editor=frame.locator('.cm-content[contenteditable="true"]')
  await editor.click();await editor.press('Control+End');await page.keyboard.insertText('\nDraft survives resource revocation.\n')
  await page.waitForFunction(async()=>Boolean((await policyClient.request('state')).dirty))
  const before=await request('state')
  await writeFile(policy,JSON.stringify([shared]))
  const granted=await ready(1), firstAlias=await ready(2)
  assert.equal(granted.asset.width,2)
  await request('referenceImageFrame',{id:1,imageRevision:granted.imageRevision,frame:0})
  await unlink(alias);await symlink(join(shared,'second.png'),alias)
  const secondAlias=await ready(2)
  assert.notEqual(firstAlias.imageRevision,secondAlias.imageRevision)
  await assert.rejects(request('referenceImageFrame',{id:2,imageRevision:firstAlias.imageRevision,frame:0}),/\(409\)/)
  await writeFile(policy,'[]')
  await denied('referenceImageInfo',{id:1})
  await denied('referenceImageFrame',{id:1,imageRevision:granted.imageRevision,frame:0})
  await denied('referenceImageInfo',{id:2})
  await ready(0)
  const revoked=await request('state')
  assert.equal(revoked.documentId,before.documentId);assert.equal(revoked.dirty,true)
  assert.match(await editor.innerText(),/Draft survives resource revocation/)
  await request('save')
  assert.match(await readFile(source,'utf8'),/Draft survives resource revocation/)
  assert.equal((await request('state')).dirty,false)
  await request('setView',{view:'preview'})
  await ready(0)
  assert.deepEqual(errors,[])
  await page.evaluate(()=>policyClient.dispose())
  console.log(JSON.stringify({ok:true,dynamicGrant:true,cachedRevocation:true,retargetedSnapshot:true,draftPreservedAndSaved:true,browserVersion:browser.version(),artifacts:directory}))
} finally {
  await browser?.close()
  if(child.exitCode==null&&child.signalCode==null){const exited=once(child,'exit');child.stdin.end('\n');await exited;assert.equal(child.exitCode,0,stderr)}
  host.closeAllConnections();host.close()
  assert.deepEqual(await readdir(cache),[])
}
