// Run against a built Studio binary with an isolated, offline app-server fixture.
// No live user profile, workspace, credentials or AI calls are involved.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {mkdir,writeFile,readFile,mkdtemp,readdir} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
import {tmpdir} from 'node:os'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url))
const binary=process.env.STUDIO_REMOTE_BINARY || join(repo,'target/release/codex-thread-studio')
const dir=await mkdtemp(join(tmpdir(),'studio-reader-expiry-')),project=join(dir,'project')
await mkdir(project)
const path=join(project,'document.md'),original=Buffer.from('# Reader lease fixture\n\nUnchanged source.\n')
await writeFile(path,original)
const env={...process.env,PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:join(repo,'scripts/fixtures/reader-comments-codex.py'),CODEX_THREAD_STUDIO_CODEX_CONNECTION:'embedded',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const server=spawn(binary,['--serve','--listen','127.0.0.1:0'],{cwd:repo,env,stdio:['ignore','pipe','pipe']})
let logs='',browser
const report={dir,checks:[]},pause=ms=>new Promise(r=>setTimeout(r,ms))
server.stdout.on('data',chunk=>logs+=chunk);server.stderr.on('data',()=>{})
try {
 let found;const deadline=Date.now()+30000
 while(!(found=logs.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/))){
  if(Date.now()>deadline||server.exitCode!=null)throw Error('Fixture server did not start')
  await pause(40)
 }
 const url=found[0],authorization='Bearer '+new URL(url).hash.slice(7)
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:1000}}),leases=new Set(),gone=[]
 page.on('response',async response=>{
  const pathname=new URL(response.url()).pathname
  if(pathname==='/studio/reader/open'&&response.ok())leases.add((await response.json()).reader.id)
  if(response.status()===410&&pathname.endsWith('/api/capabilities'))gone.push(pathname)
 })
 await page.clock.install()
 await page.goto(url)
 const row=page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]')
 await row.waitFor();await row.click()
 await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'))
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
 await page.waitForFunction(()=>!document.querySelector('#artifact-edit').disabled)
 const served=await page.request.get(new URL('/large-markdown-host.mjs',url).href)
 assert.equal(await served.text(),await readFile(join(repo,'ui/large-markdown-host.mjs'),'utf8'))
 await page.locator('#artifact-edit').click()
 const editor=page.frameLocator('.artifact-large-markdown-frame').locator('.cm-content[contenteditable="true"]')
 await editor.waitFor();await editor.click();await page.keyboard.type('Unsaved draft ')
 await page.waitForFunction(()=>document.querySelector('#artifact-title').textContent.includes('•'))
 const frameURL=await page.locator('.artifact-large-markdown-frame').getAttribute('src')
 const id=new URL(frameURL).pathname.split('/')[2]
 assert.ok(leases.has(id))
 const closed=await page.request.post(new URL('/studio/markdown/close',url).href,{headers:{Authorization:authorization},data:{id}})
 assert.equal(closed.status(),200)
 // The close route exercises the same real 410 gateway response as idle sweep.
 // No response interception or application-state mutation is used.
 await page.evaluate(()=>window.dispatchEvent(new Event('pageshow')))
 await page.waitForFunction(()=>!document.querySelector('#artifact-error').classList.contains('hidden'))
 assert.match(await page.locator('#artifact-error-message').innerText(),/Refresh to reopen|刷新文档/)
 assert.equal(await page.locator('#artifact-save').isDisabled(),true)
 assert.equal(gone.length,1)
 for(let i=0;i<5;i++){await page.clock.fastForward(30000);await pause(40)}
 assert.equal(gone.length,1)
 report.checks.push('one terminal 410; no more keepalives across five intervals; visible reopen message and disabled Save')
 let confirmation=''
 page.once('dialog',async dialog=>{confirmation=dialog.message();await dialog.dismiss()})
 await page.locator('#refresh-artifact').click()
 assert.match(confirmation,/unsaved|未保存|尚未保存/i)
 assert.equal(await page.locator('.artifact-large-markdown-frame').getAttribute('src'),frameURL)
 assert.equal(leases.size,1)
 report.checks.push('declining refresh retains the dirty reader without opening a replacement')
 page.once('dialog',dialog=>dialog.accept())
 await page.locator('#refresh-artifact').click()
 await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled)
 assert.notEqual(await page.locator('.artifact-large-markdown-frame').getAttribute('src'),frameURL)
 assert.equal(leases.size,2)
 assert.equal(await page.locator('#artifact-error').isHidden(),true)
 await page.evaluate(()=>window.dispatchEvent(new Event('online')))
 await pause(100);assert.equal(gone.length,1)
 report.checks.push('confirmed refresh opens a new working lease; old host stays stopped')
 await page.locator('#close-artifact').click()
 for(const lease of leases)await page.request.post(new URL('/studio/markdown/close',url).href,{headers:{Authorization:authorization},data:{id:lease}})
 const cleanupDeadline=Date.now()+10000
 while((await readdir(env.READER_CACHE_DIR).catch(()=>[])).length){if(Date.now()>cleanupDeadline)throw Error('Fixture reader cache not cleaned');await pause(50)}
 assert.deepEqual(await readFile(path),original)
 report.checks.push('source bytes unchanged and owned reader cache cleaned')
 report.status='passed'
} catch(error) {
 report.status='failed';report.error=String(error);report.stack=error.stack;process.exitCode=1
 const page=browser?.contexts()[0]?.pages()[0]
 await page?.screenshot({path:join(dir,'failure.png')}).catch(()=>{})
} finally {
 await browser?.close();server.kill('SIGTERM')
 await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(3000)])
 if(server.exitCode==null)server.kill('SIGKILL')
 await writeFile(join(dir,'report.json'),JSON.stringify(report,null,2)+'\n')
 console.log(JSON.stringify(report,null,2))
}
