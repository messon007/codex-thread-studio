// Actual standalone reader UI + persistent Rust storage, no frontend mocks.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, realpath, writeFile, readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const dir=await mkdtemp(join(await realpath('target'),'comment-notes-')),cache=join(dir,'cache'),file=join(dir,'notes.md')
const original='# Comment anchors\r\n\r\nrepeat 中文😀 **inline** repeat 中文😀 tail.\r\n\r\n## Table\r\n\r\n| name | value |\r\n|---|---|\r\n'+Array.from({length:98},(_,n)=>`| row ${n} repeat repeat | ${n} |\r\n`).join('')+'\r\n## Following\r\n\r\n'+'Later paragraph with content.\r\n\r\n'.repeat(1200)+'\r\n```\r\n'+Array.from({length:12},(_,n)=>`code line ${n} ${'wide '.repeat(90)}${n===11?'saved_code_target':''}\r\n`).join('')+'```\r\n'
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
    await page.getByRole('button',{name:'Save comment',exact:true}).click();await page.locator('.notes-dialog').waitFor({state:'detached'})
    await page.evaluate(()=>getSelection().removeAllRanges())
    await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
    if(writerExit){await writerExit;check(heldWriter.exitCode,0);heldWriter=null;check(busyResponses>0,true)}
  }
  const openNote=async note=>{
    await page.locator('.notes-open').click()
    // Notes deliberately remembers the previous category, including an empty
    // Favorites list after clearing it. Select the record's category explicitly.
    await page.locator('.notes-filters').getByRole('button',{name:note.kind==='favorite'?'Favorites':'Comments',exact:true}).click()
    await page.locator(`.notes-list [data-note-id="${note.id}"]`).click()
  }
  const goNote=async note=>{
    await openNote(note)
    await page.getByRole('button',{name:'Go to passage',exact:true}).click()
    await page.locator('.notes-dialog').waitFor({state:'detached'})
  }
  const assertPassage=async note=>{
    check(await page.evaluate(async segment=>{
      const selection=getSelection(),range=selection?.rangeCount?selection.getRangeAt(0):null
      if(!range)return null
      const node=range.startContainer.parentElement.closest('article[data-page],td,.table-cell-full')
      const {commentTextMap,commentRangeAt}=await import('/comment-ranges.mjs')
      const expected=commentRangeAt(commentTextMap(node),segment.startUtf16,segment.endUtf16)
      return {text:range.toString(),same:range.compareBoundaryPoints(Range.START_TO_START,expected)===0&&range.compareBoundaryPoints(Range.END_TO_END,expected)===0}
    },note.anchor.textRange.segments[0]),{text:note.quote,same:true})
  }
  await page.goto(url.href);await ready()
  await select('article[data-page="0"] p','repeat 中文😀',true);await save('Second occurrence',true)
  let notes=await api({operation:'list',offset:0,kind:'comment'})
  const first=notes.items[0];check(notes.total,1);check(first.quote,'repeat 中文😀');check(first.anchor.textRange.version,1)
  check(await page.evaluate(()=>[...CSS.highlights].filter(([key])=>key.startsWith('marknoria-comments-')).flatMap(([,value])=>[...value]).map(range=>range.toString())),[first.quote])
  const firstOffset=first.anchor.textRange.segments[0].startUtf16;assert.ok(firstOffset>20);assertions++
  await page.reload();await ready();await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
  check((await api({operation:'list',offset:0,kind:'comment'})).items[0].id,first.id)
  // Ordinary real pointer activation, edit/save, then reopen after a reload.
  const clickStoredMark=async()=>{
    const point=await page.locator('article[data-page="0"]').evaluate(async(article,segment)=>{
      const {commentTextMap,commentRangeAt}=await import('/comment-ranges.mjs')
      const range=commentRangeAt(commentTextMap(article),segment.startUtf16,segment.startUtf16+2)
      if(!range)throw Error('Stored comment no longer resolves in unchanged source')
      const r=range.getBoundingClientRect();return {x:r.left+r.width/2,y:r.top+r.height/2}
    },first.anchor.textRange.segments[0])
    await page.mouse.click(point.x,point.y)
    await page.getByRole('textbox',{name:'Comment',exact:true}).waitFor()
  }
  await clickStoredMark()
  check(await page.getByRole('textbox',{name:'Comment',exact:true}).inputValue(),'Second occurrence')
  await page.getByRole('textbox',{name:'Comment',exact:true}).fill('Edited persisted occurrence')
  await page.getByRole('button',{name:'Save comment',exact:true}).click()
  await page.locator('.notes-dialog').waitFor({state:'detached'})
  await page.reload();await ready();await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
  await clickStoredMark()
  check(await page.getByRole('textbox',{name:'Comment',exact:true}).inputValue(),'Edited persisted occurrence')
  check((await api({operation:'list',offset:0,kind:'comment'})).items.find(note=>note.id===first.id).comment,'Edited persisted occurrence')
  await page.locator('.notes-dialog .dialog-close').click()
  // Notes returns to the exact repeated occurrence from a remote reading page.
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.pages.length-1)})
  await goNote(first);await assertPassage(first)
  await page.evaluate(()=>getSelection().removeAllRanges())
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
  await goNote(tableNote);await assertPassage(tableNote)
  await page.waitForFunction(()=>Number(document.querySelector('.reader-shell').dataset.commentMarks)>0)
  // The full-cell viewer and the table cell share the same bounded text scope.
  await page.evaluate(()=>getSelection().removeAllRanges())
  await page.locator('article[data-table="0"]').getByRole('button',{name:'View full cell',exact:true}).click()
  await select('.table-cell-full','repeat',true);await save('Full cell partial word')
  const fullCellNote=(await api({operation:'list',offset:0,kind:'comment'})).items.find(n=>n.comment==='Full cell partial word')
  await page.locator('.table-cell-dialog .dialog-close').click()
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.pages.length-1)})
  await goNote(fullCellNote);await assertPassage(fullCellNote)
  // A host may constrain a code block to its own two-axis scroller. The saved
  // occurrence must be revealed inside that scroller as well as the reader.
  await page.addStyleTag({content:'.page pre { white-space:pre; overflow:auto; max-height:120px; }'})
  await page.evaluate(async()=>{await readerExperiment.jump(readerExperiment.index.pages.length-1)})
  const codeBlock=page.locator('article pre').filter({hasText:'saved_code_target'})
  await codeBlock.evaluate(async node=>{
    node.scrollIntoView({block:'center'});node.scrollTop=node.scrollHeight;node.scrollLeft=node.scrollWidth
    let previous='',stable=0
    for(let attempt=0;attempt<60;attempt++){
      await new Promise(resolve=>requestAnimationFrame(resolve))
      const rect=node.getBoundingClientRect(),signature=[rect.x,rect.y,rect.width,rect.height,node.scrollLeft,node.scrollTop,document.querySelector('#viewport').scrollTop].join(':')
      stable=signature===previous?stable+1:0;previous=signature
      if(stable>=4&&node.isConnected)return
    }
    throw Error('Code-scroller geometry did not settle before selection')
  })
  await select('article pre','saved_code_target');await save('Code scroller passage')
  const codeNote=(await api({operation:'list',offset:0,kind:'comment'})).items.find(n=>n.comment==='Code scroller passage')
  await page.evaluate(async()=>{await readerExperiment.jump(0)})
  await goNote(codeNote);await assertPassage(codeNote)
  check(await page.evaluate(()=>{
    const range=getSelection().getRangeAt(0),pre=range.startContainer.parentElement.closest('pre'),rect=range.getBoundingClientRect(),box=pre.getBoundingClientRect(),viewport=document.querySelector('#viewport').getBoundingClientRect()
    return pre.scrollLeft>0&&pre.scrollTop>0&&rect.left>=box.left-1&&rect.right<=box.right+1&&rect.top>=box.top-1&&rect.bottom<=box.bottom+1&&rect.top>=viewport.top-1&&rect.bottom<=viewport.bottom+1
  }),true)
  // Source stores original UTF-8 bytes and paints without stealing selection.
  await page.locator('#source-mode').click();await page.waitForFunction(()=>readerExperiment.source?.code&&document.querySelector('#source-view').dataset.ready==='true')
  await page.evaluate(async()=>{await readerExperiment.source.go(1);const code=readerExperiment.source.code,text=code.text,from=text.lastIndexOf('repeat 中文😀',150);await code.select(from,from+'repeat 中文😀'.length);document.dispatchEvent(new Event('selectionchange'))})
  await page.locator('.notes-toolbar:not([hidden])').waitFor();await save('Original source bytes')
  notes=await api({operation:'list',offset:0,kind:'comment'});const sourceNote=notes.items.find(n=>n.comment==='Original source bytes')
  check(sourceNote.anchor.location.kind,'source');check(Buffer.from(original).subarray(sourceNote.anchor.location.startByte,sourceNote.anchor.location.endByte).toString(),sourceNote.quote)
  // Keep a different active search match in the same bounded Source window.
  // Returning to the note must not let a search repaint replace its selection.
  await page.locator('#search-toggle').click();await page.locator('#query').fill('tail');await page.locator('#query').press('Enter')
  await page.locator('#matches button').first().click()
  await page.waitForFunction(()=>{const code=readerExperiment.source.code,range=code.view.state.selection.main;return code.text.slice(range.from,range.to)==='tail'})
  await page.locator('#preview-mode').click()
  await page.waitForFunction(()=>!document.querySelector('#preview-mode').disabled&&document.querySelector('#preview-mode').getAttribute('aria-pressed')==='true'&&!readerExperiment.source.visible)
  await goNote(sourceNote)
  check(await page.evaluate(()=>{const code=readerExperiment.source.code,range=code.view.state.selection.main;return code.text.slice(range.from,range.to)}),sourceNote.quote)
  if (!await page.locator('#search').isVisible()) await page.locator('#search-toggle').click()
  await page.locator('#search-next').click()
  await page.waitForFunction(()=>{const code=readerExperiment.source.code,range=code.view.state.selection.main;return code.text.slice(range.from,range.to)==='tail'})
  await page.locator('#edit-mode').click()
  await page.waitForFunction(()=>readerExperiment.editor?.visible&&!readerExperiment.editor.busy&&!document.querySelector('#edit-mode').disabled)
  await goNote(sourceNote)
  check(await page.evaluate(()=>{const code=readerExperiment.source.code,range=code.view.state.selection.main;return {source:readerExperiment.source.visible,editing:readerExperiment.editor.visible,text:code.text.slice(range.from,range.to)}}),{source:true,editing:false,text:sourceNote.quote})
  // A structurally valid selector with a wrong fingerprint fails closed and
  // leaves the manager open instead of jumping to a repeated quote by guess.
  const mismatched=structuredClone(first.anchor);mismatched.textRange.segments[0].textHash='f'.repeat(64)
  await api({operation:'add',kind:'comment',quote:first.quote,comment:'Unresolvable fingerprint',anchor:mismatched,revision:first.revision})
  const failed=(await api({operation:'list',offset:0,kind:'comment'})).items.find(note=>note.comment==='Unresolvable fingerprint')
  await openNote(failed);await page.getByRole('button',{name:'Go to passage',exact:true}).click()
  await page.getByText('The saved passage no longer matches the rendered text',{exact:true}).waitFor()
  check(await page.locator('.notes-dialog').count(),1)
  await page.locator('.notes-dialog .dialog-close').click()
  await api({operation:'delete',id:failed.id})
  await page.locator('.notes-open').click();await page.waitForFunction(()=>document.querySelector('.notes-footer')?.textContent.includes('saved item'))
  await page.locator('.notes-filters').getByRole('button',{name:'Comments',exact:true}).click()
  await page.getByRole('button',{name:'Clear comments',exact:true}).click()
  await page.locator('.notes-confirm').getByRole('button',{name:'Clear all',exact:true}).click()
  await page.locator('.notes-empty').waitFor();await page.locator('.notes-dialog .dialog-close').click();await marked(0)
  check((await api({operation:'list',offset:0,kind:'comment'})).total,0)
  check(await readFile(file,'utf8'),original);check(pageErrors,[])
  await page.screenshot({path:join(dir,'checked.png')})
  console.log(JSON.stringify({ok:true,assertions,persistence:true,tablePaging:true,sourceBytes:true,navigation:true,exactRepeatedPassage:true,failedNavigationRetainsNotes:true,overlaps:true,categoryClear:true,busyRetry:busyResponses>0,artifacts:dir}))
}catch(error){console.error('Comment Notes artifacts:',dir,await activePage?.evaluate(()=>({status:[...document.querySelectorAll('[role=status]')].map(n=>n.textContent),marksError:document.querySelector('.reader-shell')?.dataset.commentMarksError})));await activePage?.screenshot({path:join(dir,'failure.png')});throw error}finally{if(heldWriter?.exitCode==null)heldWriter?.kill('SIGTERM');await browser?.close();if(child.exitCode==null){const exited=once(child,'exit');child.kill('SIGTERM');await exited}}
