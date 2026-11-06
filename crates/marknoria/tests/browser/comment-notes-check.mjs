// Actual standalone reader UI + persistent Rust storage, no frontend mocks.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, realpath, writeFile, readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const dir=await mkdtemp(join(await realpath('target'),'comment-notes-')),cache=join(dir,'cache'),file=join(dir,'notes.md')
const original='# Comment anchors\r\n\r\nrepeat 中文😀 **inline** repeat 中文😀 tail.\r\n\r\n## Table\r\n\r\n| name | value |\r\n|---|---|\r\n'+Array.from({length:98},(_,n)=>`| row ${n} repeat repeat | ${n} |\r\n`).join('')+'\r\n## Following\r\n\r\n'+'Later paragraph with content.\r\n\r\n'.repeat(1200)
await mkdir(cache);await writeFile(file,original)
const child=spawn(resolve(process.argv[2]||'target/debug/marknoria-server'),[file,'--root',dir],{env:{...process.env,READER_CACHE_DIR:cache,MARKNORIA_CONFIG_DIR:join(dir,'config')},stdio:['ignore','pipe','pipe']})
let output='',errors='',browser,activePage,heldWriter,busyResponses=0,assertions=0
child.stdout.on('data',bytes=>output+=bytes);child.stderr.on('data',bytes=>errors+=bytes)
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),check=(a,b)=>{assert.deepEqual(a,b);assertions++}
try{
  const deadline=Date.now()+30000;while(!output.includes('http://')&&child.exitCode==null&&Date.now()<deadline)await pause(20)
  assert.match(output,/http:\/\/127\.0\.0\.1:/,errors)
  const url=new URL(output.trim().split('\n')[0]),token=url.hash.slice(1)
  const api=async request=>{const response=await fetch(new URL('/api/notes',url),{method:'POST',headers:{'x-reader-token':token,'Content-Type':'application/json'},body:JSON.stringify(request)});assert.equal(response.status,200,await response.clone().text());return response.json()}
  const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
  browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const page=await browser.newPage({viewport:{width:1400,height:900}}),pageErrors=[]
  activePage=page
  page.on('pageerror',error=>pageErrors.push(String(error)))
  page.on('response',response=>{if(response.url().endsWith('/api/notes')&&response.status()===429)busyResponses++})
  const ready=()=>page.waitForFunction(()=>readerExperiment?.metrics.firstContentMs!=null)
  const marked=count=>page.waitForFunction(count=>Number(document.querySelector('.reader-shell').dataset.commentMarks||0)===count,count)
  const select=async(selector,word,last=false)=>{
    await page.locator(selector).first().evaluate((node,{word,last})=>{
      const walker=document.createTreeWalker(node,NodeFilter.SHOW_TEXT);let child,target,offset
      while((child=walker.nextNode())){const found=last?child.data.lastIndexOf(word):child.data.indexOf(word);if(found>=0){target=child;offset=found;if(!last)break}}
      if(!target)throw Error('Selection fixture missing')
      const range=document.createRange();range.setStart(target,offset);range.setEnd(target,offset+word.length)
      getSelection().removeAllRanges();getSelection().addRange(range);document.dispatchEvent(new Event('selectionchange'))
    },{word,last})
    await page.locator('.notes-toolbar:not([hidden])').waitFor()
  }
  const save=async(text,contended=false)=>{
    await page.locator('.notes-toolbar').getByRole('button',{name:'Comment',exact:true}).click()
    await page.getByRole('textbox',{name:'Comment',exact:true}).fill(text)
    let writerExit
    if(contended){
      // Hold only this private notebook's writer. The real UI must retry HTTP
      // 429 and save exactly once after the lock is released, without a 400.
      heldWriter=spawn('python3',['-c','import sqlite3,sys,time; db=sqlite3.connect(sys.argv[1]); db.execute("BEGIN IMMEDIATE"); print("locked",flush=True); time.sleep(3.2); db.commit()',join(dir,'config','notes.sqlite')],{stdio:['ignore','pipe','inherit']})
      writerExit=once(heldWriter,'exit');let locked='';heldWriter.stdout.on('data',bytes=>locked+=bytes)
      const until=Date.now()+5000;while(!locked.includes('locked')&&heldWriter.exitCode==null&&Date.now()<until)await pause(10)
      assert.match(locked,/locked/)
    }
    await page.getByRole('button',{name:'Save comment',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.notes-footer')?.textContent.includes('saved item'))
    await page.locator('.notes-dialog .dialog-close').click();await page.evaluate(()=>getSelection().removeAllRanges())
    await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
    if(writerExit){await writerExit;check(heldWriter.exitCode,0);heldWriter=null;check(busyResponses>0,true)}
  }
  await page.goto(url.href);await ready()
  await select('article[data-page="0"] p','repeat 中文😀',true);await save('Second occurrence',true)
  let notes=await api({operation:'list',offset:0,kind:'comment'})
  const first=notes.items[0];check(notes.total,1);check(first.quote,'repeat 中文😀');check(first.anchor.textRange.version,1)
  check(await page.evaluate(()=>[...CSS.highlights].filter(([key])=>key.startsWith('marknoria-comments-')).flatMap(([,value])=>[...value]).map(range=>range.toString())),[first.quote])
  const firstOffset=first.anchor.textRange.segments[0].startUtf16;assert.ok(firstOffset>20);assertions++
  await page.reload();await ready();await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
  check((await api({operation:'list',offset:0,kind:'comment'})).items[0].id,first.id)
  // Overlap and Favorites clear use the actual independent category controls.
  await select('article[data-page="0"] p','repeat 中文😀',true);await save('Overlapping occurrence')
  await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>=2)
  await select('article[data-page="0"] p','repeat 中文😀',true)
  await page.locator('.notes-toolbar').getByRole('button',{name:'Favorite',exact:true}).click()
  await page.locator('.notes-open').click();await page.waitForFunction(()=>document.querySelector('.notes-footer')?.textContent.includes('saved item'))
  await page.locator('.notes-filters').getByRole('button',{name:'Favorites',exact:true}).click()
  await page.getByRole('button',{name:'Clear favorites',exact:true}).click()
  await page.locator('.notes-confirm').getByRole('button',{name:'Clear all',exact:true}).click()
  await page.locator('.notes-empty').waitFor();await page.locator('.notes-dialog .dialog-close').click()
  check((await api({operation:'list',offset:0,kind:'comment'})).total,2)
  await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>=2)
  // An exact cell range survives independent row-window replacement.
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.tables[0].firstPage,null,75);await readerExperiment.tables.go(0,75,0)})
  const cell='article[data-table="0"] tbody tr:nth-child(26) td:first-child'
  await select(cell,'repeat',true);await page.locator(cell).first().evaluate(node=>node.click());await save('Table partial word')
  notes=await api({operation:'list',offset:0,kind:'comment'});const tableNote=notes.items.find(n=>n.comment==='Table partial word')
  check(tableNote.anchor.textRange.segments[0].scope.row,75);check(tableNote.anchor.location.column,0)
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.tables[0].firstPage,null,0);await readerExperiment.tables.go(0,0,0)})
  await marked(0)
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.tables[0].firstPage,null,75);await readerExperiment.tables.go(0,75,0)})
  await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
  // Source stores original UTF-8 bytes and paints without stealing selection.
  await page.locator('#source-mode').click();await page.waitForFunction(()=>readerExperiment.source?.code&&document.querySelector('#source-view').dataset.ready==='true')
  await page.evaluate(async()=>{await readerExperiment.source.go(1);const code=readerExperiment.source.code,text=code.text,from=text.lastIndexOf('repeat 中文😀',150);await code.select(from,from+'repeat 中文😀'.length);document.dispatchEvent(new Event('selectionchange'))})
  await page.locator('.notes-toolbar:not([hidden])').waitFor();await save('Original source bytes')
  notes=await api({operation:'list',offset:0,kind:'comment'});const sourceNote=notes.items.find(n=>n.comment==='Original source bytes')
  check(sourceNote.anchor.location.kind,'source');check(Buffer.from(original).subarray(sourceNote.anchor.location.startByte,sourceNote.anchor.location.endByte).toString(),sourceNote.quote)
  await page.locator('.notes-open').click();await page.waitForFunction(()=>document.querySelector('.notes-footer')?.textContent.includes('saved item'))
  await page.locator('.notes-filters').getByRole('button',{name:'Comments',exact:true}).click()
  await page.getByRole('button',{name:'Clear comments',exact:true}).click()
  await page.locator('.notes-confirm').getByRole('button',{name:'Clear all',exact:true}).click()
  await page.locator('.notes-empty').waitFor();await page.locator('.notes-dialog .dialog-close').click();await marked(0)
  check((await api({operation:'list',offset:0,kind:'comment'})).total,0)
  check(await readFile(file,'utf8'),original);check(pageErrors,[])
  await page.screenshot({path:join(dir,'checked.png')})
  console.log(JSON.stringify({ok:true,assertions,persistence:true,tablePaging:true,sourceBytes:true,overlaps:true,categoryClear:true,busyRetry:busyResponses>0,artifacts:dir}))
}catch(error){console.error('Comment Notes artifacts:',dir,await activePage?.evaluate(()=>({status:[...document.querySelectorAll('[role=status]')].map(n=>n.textContent),marksError:document.querySelector('.reader-shell')?.dataset.commentMarksError})));await activePage?.screenshot({path:join(dir,'failure.png')});throw error}finally{if(heldWriter?.exitCode==null)heldWriter?.kill('SIGTERM');await browser?.close();if(child.exitCode==null){const exited=once(child,'exit');child.kill('SIGTERM');await exited}}
