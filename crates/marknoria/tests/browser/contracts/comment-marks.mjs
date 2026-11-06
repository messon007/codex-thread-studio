export async function commentMarksContract(moduleUrl) {
  if(moduleUrl.endsWith('#fallback'))Object.defineProperty(CSS,'highlights',{configurable:true,value:undefined})
  const base=new URL('.',new URL(moduleUrl,location.href))
  const { mountCommentMarks } = await import(moduleUrl)
  const { captureProjectionComment, commentTextMap, commentRangeAt, commentTextHash, commentScopes } = await import(new URL('comment-ranges.mjs',base).href)
  const { sourceCommentContext } = await import(new URL('source-comment-marks.mjs',base).href)
  const { codeView } = await import(new URL('vendor/editor.mjs',base).href)
  const { mountTableCells } = await import(new URL('table-cells.mjs',base).href)
  const owner = { documentId:'document-a',revision:'revision-a' }
  const root = document.createElement('section')
  root.style.cssText = 'position:fixed;inset:10px;overflow:auto;background:white;line-height:30px'
  document.body.append(root)
  let assertions = 0, marks, other, table, source, result = [], calls = 0, opened = null, late, lateSignal, error = null
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const wait = async predicate => { const deadline=performance.now()+8000; while(!predicate()){ if(performance.now()>deadline)throw Error('Comment marker timeout: '+error); await new Promise(resolve=>setTimeout(resolve,20)) } }
  const article = document.createElement('article')
  article.dataset.page='0'; article.dataset.marknoriaProse=''
  article.innerHTML='<p>same 中文😀 <em>cross inline</em> same 中文😀 tail</p>'
  root.append(article)
  const text = article.textContent, map = commentTextMap(article), start=text.lastIndexOf('same'), end=start+'same 中文😀'.length
  const range = commentRangeAt(map,start,end)
  const selection = document.getSelection(); selection.removeAllRanges(); selection.addRange(range)
  const captured=await captureProjectionComment(root,range,{kind:'line',line:1})
  check(captured.version===1 && captured.segments[0].startUtf16===start,'Second duplicate must retain its exact original offset')
  check(captured.segments[0].endUtf16===end && captured.segments[0].textHash.length===64,'Unicode endpoints and fingerprint lost')
  const note={...owner,id:crypto.randomUUID(),kind:'comment',quote:range.toString(),anchor:{location:{kind:'line',line:1},textRange:captured}}
  result=[note]
  const storage=async(request,{signal})=>{
    calls++
    if(late===true){lateSignal=signal;return new Promise(resolve=>late=resolve)}
    const matching=result.filter(note=>note.anchor.textRange?.segments.some(s=>request.scopes.includes(`p:${s.scope.page}`))||note.anchor.location.kind==='source')
    return { revision:owner.revision,items:matching.slice(request.offset,request.offset+64),nextOffset:Math.min(matching.length,request.offset+64),complete:request.offset+64>=matching.length }
  }
  const snapshots=()=>[...CSS.highlights.entries()].filter(([name])=>name.startsWith('marknoria-comments-')).flatMap(([,value])=>[...value])
  const hasNative=Boolean(CSS.highlights&&window.Highlight)
  const registrySize=()=>hasNative?snapshots().length:document.querySelectorAll('[data-comment-rect]').length
  try {
    marks=mountCommentMarks({owner,root,load:storage,open:id=>opened=id,onStatus:value=>error=value})
    await wait(()=>marks.state().rectangles>0)
    check(article.textContent===text && !article.querySelector('[data-comment-ids]'),'Marker rewrote the document/EPUB CFI tree')
    check(selection.toString()===note.quote,'Painting changed the live text selection')
    if(hasNative){check(snapshots()[0].startOffset===range.startOffset,'Wrong duplicate was underlined');check(snapshots()[0].toString()===note.quote,'Wrong comment quote painted')}
    check(registrySize()>0,'No actual painted geometry or CSS range')
    selection.removeAllRanges(); marks.refresh();await new Promise(resolve=>setTimeout(resolve,100))
    check(marks.state().rectangles>0,'Collapsing selection removed a durable mark')
    const prior=calls;for(let i=0;i<10;i++)root.dispatchEvent(new Event('scroll'))
    await new Promise(resolve=>setTimeout(resolve,180));check(calls===prior,'Scrolling reread the notebook despite identical scopes')
    result.push({...note,id:crypto.randomUUID()});marks.refresh(true);await wait(()=>marks.state().records===2)
    check(marks.state().rectangles>=2,'Overlapping comments were lost')
    result.shift();marks.refresh(true);await wait(()=>marks.state().records===1)
    check(marks.state().rectangles>0,'Deleting one overlap removed the other')
    const original=article.innerHTML
    article.innerHTML='<p>prefix '+article.textContent+'</p>';marks.refresh();await wait(()=>marks.state().rectangles===0)
    check(article.textContent.includes(note.quote),'Stale hash test lost its duplicate quote')
    article.innerHTML=original;marks.refresh();await wait(()=>marks.state().rectangles>0)
    check(article.textContent===text,'Recreated immutable page failed')
    const box=commentRangeAt(commentTextMap(article),start,end).getBoundingClientRect()
    document.dispatchEvent(new PointerEvent('pointermove',{clientX:box.left+2,clientY:box.top+4}))
    await wait(()=>!document.querySelector('.comment-mark-popup').hidden)
    document.dispatchEvent(new PointerEvent('pointermove',{clientX:box.left+2,clientY:box.bottom+2}))
    await new Promise(resolve=>setTimeout(resolve,30))
    check(!document.querySelector('.comment-mark-popup').hidden,'Crossing the popup gap hid its action')
    document.querySelector('.comment-mark-popup button').click()
    check(opened===result[0].id,'Explicit comment action did not open associated note')
    marks.dispose();check(registrySize()===0 && !document.querySelector('[data-comment-overlay]'),'Dispose leaked owned marks')
    const beforeHost=calls
    marks=mountCommentMarks({owner,root,onStatus:value=>error=value})
    marks.replace(result);await wait(()=>marks.state().rectangles>0)
    check(calls===beforeHost,'Explicit host marks performed a storage read')
    let rejected=false;try{marks.replace([{...note,revision:'wrong'}])}catch{rejected=true}check(rejected,'Host replacement accepted a different revision')
    rejected=false;try{marks.replace(Array.from({length:513},()=>note))}catch{rejected=true}check(rejected,'Host replacement accepted an unbounded record set')
    rejected=false;try{marks.replace([{...note,anchor:{location:{kind:'pdf',pages:{}}}}])}catch{rejected=true}check(rejected,'Malformed PDF anchor accepted')
    rejected=false;try{marks.replace([{...note,anchor:true}])}catch{rejected=true}check(rejected,'Non-object anchor accepted')
    rejected=false;try{marks.replace([note,note])}catch{rejected=true}check(rejected,'Duplicate host mark IDs accepted')
    marks.replace([]);await wait(()=>marks.state().rectangles===0);marks.dispose()

    // The same registered table component captures and resolves partial cell
    // selections, independently of visible-column order or result pagination.
    article.dataset.table='2';article.innerHTML='<div class="tools"></div><div class="content"><table><tbody><tr><td>repeat repeat 中文😀</td></tr></tbody></table></div>'
    const cell=article.querySelector('td'),location={kind:'cell',table:2,row:75,column:9}
    table=mountTableCells(article.querySelector('.content'),{controls:article.querySelector('.tools'),locate:()=>location})
    table.update();table.select(cell)
    const cellRange=commentRangeAt(commentTextMap(cell),7,13);selection.removeAllRanges();selection.addRange(cellRange)
    const selected=await table.captureSelection({textOnly:true})
    check(selected.quote==='repeat'&&selected.textRange.segments[0].scope.column===9&&selected.textRange.segments[0].scope.row===75,'Cell selection lost source column/row identity')
    check(selected.textRange.segments[0].startUtf16===7,'Cell duplicate was guessed')
    marks=mountCommentMarks({owner,root,onStatus:value=>error=value});marks.replace([{...note,quote:selected.quote,anchor:{location,textRange:selected.textRange}}]);await wait(()=>marks.state().rectangles>0)
    check(commentScopes(root).some(value=>value.node===cell),'Registered cell missing from bounded scopes')
    const elementRange=document.createRange();elementRange.selectNodeContents(cell)
    const elementSelection=await captureProjectionComment(root,elementRange,location)
    check(elementSelection?.segments[0].startUtf16===0&&elementSelection.segments[0].endUtf16===cell.textContent.length,'Element-boundary cell selection lost exact selector')
    article.querySelector('.table-cell-view').click()
    await wait(()=>document.querySelector('.table-cell-dialog')?.open)
    if(!hasNative)await wait(()=>document.querySelector('.table-cell-dialog [data-comment-rect]'))
    else await wait(()=>snapshots().some(range=>range.startContainer.parentElement.closest('.table-cell-full')))
    check(document.querySelector('.table-cell-full').textContent===cell.textContent,'Full-cell annotation changed source value')
    document.querySelector('.table-cell-dialog .dialog-close').click()
    if(!hasNative){
      await wait(()=>document.querySelector('[data-comment-rect]'))
      const before=parseFloat(document.querySelector('[data-comment-rect]').style.left)
      cell.style.paddingLeft='40px'
      await wait(()=>parseFloat(document.querySelector('[data-comment-rect]')?.style.left)>before+10)
      check(true,'Column geometry refresh follows style changes without changing text')
      cell.style.paddingLeft=''
    }
    cell.dispatchEvent(new MouseEvent('dblclick',{bubbles:true}));check(!document.querySelector('.table-cell-dialog'),'Marks regressed double-click word selection')
    cell.textContent='changed repeat repeat 中文😀';marks.refresh();await wait(()=>marks.state().rectangles===0)
    check(!error,'Normal stale mark should fail closed without an error')
    marks.dispose();table.dispose();table=null;article.removeAttribute('data-table')

    // Chunked fetching and disposal ignore late async results.
    article.innerHTML=original;result=Array.from({length:65},()=>({...note,id:crypto.randomUUID()}))
    marks=mountCommentMarks({owner,root,load:storage,onStatus:value=>error=value});await wait(()=>marks.state().records===65)
    check(marks.state().rectangles===65,'Paginated comment view dropped records')
    late=true;marks.refresh(true);await wait(()=>lateSignal);marks.dispose();check(lateSignal.aborted,'Dispose did not abort scoped read')
    late({revision:owner.revision,items:[note],nextOffset:1,complete:true});await new Promise(resolve=>setTimeout(resolve,100))
    check(registrySize()===0,'Late response repainted disposed reader')
    // A mutation must invalidate a pending pre-mutation snapshot immediately.
    late=true;result=[note];marks=mountCommentMarks({owner,root,load:storage});await wait(()=>typeof late==='function')
    const outdated=late,oldSignal=lateSignal;result=[];late=false;marks.refresh(true)
    check(oldSignal.aborted,'Mutation did not synchronously abort old snapshot')
    outdated({revision:owner.revision,items:[note],nextOffset:1,complete:true})
    await new Promise(resolve=>setTimeout(resolve,150));check(marks.state().records===0&&marks.state().rectangles===0,'Pre-mutation snapshot overwrote cleared marks');marks.dispose()
    // Source mapping handles BOM, CRLF, CJK and UTF-8 byte/UTF-16 differences in one bounded pass.
    article.innerHTML=''
    source=codeView(article,{readOnly:true});const raw='\ufeffprefix\r\nsame 中文😀 same 中文😀\r\nend',normalized=raw.replaceAll('\r\n','\n')
    source.load(normalized,1,'text')
    const rawStart=raw.lastIndexOf('same'),sourceStart=new TextEncoder().encode(raw.slice(0,rawStart)).length,sourceEnd=sourceStart+new TextEncoder().encode('same 中文😀').length
    const sourceNote={...note,quote:'same 中文😀',anchor:{location:{kind:'source',startByte:sourceStart,endByte:sourceEnd}}},page={start:0,end:new TextEncoder().encode(raw).length,text:raw}
    marks=mountCommentMarks({owner,root,context:()=>sourceCommentContext(source,page)})
    marks.replace([sourceNote]);await wait(()=>marks.state().rectangles>0)
    if(hasNative)check(snapshots()[0].toString()===sourceNote.quote,'Source bytes mapped to wrong rendered text')
    check(source.text===normalized,'Source markers mutated the CodeMirror document')
    check(source.view.state.selection.main.empty,'Source markers changed editor selection')
    marks.dispose();source.dispose();source=null;article.innerHTML=original
    // Per-pane ownership never deletes another reader's registry/layer.
    late=false;result=[note];marks=mountCommentMarks({owner,root});other=mountCommentMarks({owner:{...owner,documentId:'b'},root})
    marks.replace([note]);other.replace([note]);await wait(()=>marks.state().rectangles&&other.state().rectangles)
    marks.dispose();check(other.state().rectangles>0,'Disposing a pane removed another pane')
    other.dispose()
    // Fingerprints cover exact rendered text, including CJK and astral scalars.
    check(await commentTextHash(article,article.textContent)===captured.segments[0].textHash,'Stable recreated text has a different fingerprint')
    check(commentRangeAt(commentTextMap(article),text.indexOf('😀')+1,text.indexOf('😀')+2)===null,'An endpoint split a surrogate pair')
    return { assertions, nativeHighlight:hasNative }
  } finally { marks?.dispose();other?.dispose();table?.dispose();source?.dispose();root.remove() }
}
