export async function run() {
  const { createPdfReader } = await import('/pdf-reader.mjs')
  const bytes = await fetch('/fixture.pdf').then(response => response.arrayBuffer())
  const viewport = document.createElement('div')
  viewport.className = 'artifact-content'
  viewport.style.cssText = 'width:760px;height:420px;overflow:auto'
  const root = document.createElement('div')
  viewport.append(root)
  document.body.append(viewport)
  let reader, assertions = 0
  const check = (condition, message) => { if (!condition) throw Error(message); assertions++ }
  const wait = async predicate => {
    const deadline = performance.now()+15000
    while (!predicate()) { if (performance.now()>deadline) throw Error('PDF host operation timed out'); await new Promise(resolve=>setTimeout(resolve,20)) }
  }
  try {
    reader = await createPdfReader({container:root,bytes,translate:value=>value})
    check(reader.pageCount===12 && root.querySelector('.marknoria-pdf'), 'Shared PDF component was not mounted')
    check(reader.outline().length===2 && reader.outline()[1].target.page===12, 'Shared outline lost its original pages')
    await reader.settings({zoom:1,continuous:false})
    await reader.goToPage(8)
    check(reader.state().page===8 && reader.state().zoom===1 && !reader.state().continuous, 'Global zoom or paged navigation changed')
    await wait(()=>root.querySelector('[data-pdf-page="8"] .textLayer span'))
    check(root.querySelectorAll('canvas').length<=3, 'PDF mounted too many canvases')
    const node=root.querySelector('[data-pdf-page="8"] .textLayer span').firstChild
    const range=document.createRange()
    range.selectNodeContents(node)
    const selection=getSelection()
    selection.removeAllRanges();selection.addRange(range)
    const reference=reader.selection()
    check(reference?.quote.includes('PDF sentinel page 8') && reference.precision==='exact', 'PDF text selection missing')
    check(reference.location.kind==='pdf' && reference.location.pages[0].page===8 && reference.location.pages[0].rects.length>0, 'PDF page/region coordinates missing')
    check(/^[a-f0-9]{64}$/u.test(reference.revision), 'PDF revision does not identify original bytes')
    await reader.navigateReference(reference)
    check(await reader.navigateReference({...reference,revision:'0'.repeat(64)}).then(()=>false,()=>true), 'Stale PDF reference accepted')
    await reader.settings({continuous:true})
    await reader.goToPage(10)
    check(reader.state().page===10 && reader.state().zoom===1 && reader.state().continuous, 'Continuous/rotated page lost zoom')
    await reader.destroy();reader=null
    check(root.querySelectorAll('canvas,.marknoria-pdf').length===0, 'PDF retained bitmaps after close')
    reader=await createPdfReader({container:root,bytes,search:'page 12'})
    check(reader.state().page===12, 'Complete-document search did not navigate to its match')
    await reader.destroy();reader=null
    const abort=new AbortController();abort.abort()
    check(await createPdfReader({container:root,bytes,signal:abort.signal}).then(()=>false,error=>error.name==='AbortError'), 'Closed host started a PDF renderer')
    return {assertions,sharedPdf:true,typedReferences:true,boundedCanvases:true}
  } finally {
    await reader?.destroy()
    viewport.remove()
  }
}
