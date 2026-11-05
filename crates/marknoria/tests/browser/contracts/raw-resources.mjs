// Finite compatibility corpus from frozen Studio behavior, in both Web engines.
export async function rawResourcesContract(urls) {
  const {createReaderClient}=await import('/host-client.mjs')
  const {projectionHtml}=await import('/projection-html.mjs')
  const {mountReferences,readerReferenceProvider}=await import('/references.mjs')
  const {mountImageView,readerImageProvider}=await import('/image-view.mjs')
  const frames=[],clients=[],views=[],containers=[]
  let assertions=0
  const check=(value,message)=>{if(!value)throw Error(message);assertions++}
  const wait=async predicate=>{const until=performance.now()+30000;while(!predicate()){if(performance.now()>until)throw Error('Raw resource contract timed out');await new Promise(resolve=>setTimeout(resolve,25))}}
  function container(){const viewport=document.createElement('div'),root=document.createElement('div');viewport.style.cssText='width:600px;height:300px;overflow:auto';viewport.append(root);document.body.prepend(viewport);containers.push(viewport);return{viewport,root}}
  function pixels(image){const canvas=document.createElement('canvas');canvas.width=2;canvas.height=1;const context=canvas.getContext('2d');context.imageSmoothingEnabled=false;context.drawImage(image,2000,500,2,1,0,0,2,1);return [...context.getImageData(0,0,2,1).data]}
  try {
    for(const url of urls){const frame=document.createElement('iframe');frame.src=url;frame.title='Raw resource corpus';frame.style.cssText='width:600px;height:220px;display:block';document.body.append(frame);frames.push(frame);clients.push(createReaderClient(frame))}
    const owners=await Promise.all(clients.map(client=>client.ready()))
    check(owners[0].document.references===11,'Frozen raw resource count changed')
    const md=await clients[0].request('referenceInfo',{id:2}),raw=await clients[0].request('referenceInfo',{id:3}),other=await clients[0].request('referenceInfo',{id:4})
    check(md.reference.precision==='exact'&&raw.reference.precision==='approximate','Source precision was invented')
    check(raw.reference.startByte===other.reference.startByte&&raw.reference.endByte===other.reference.endByte&&raw.reference.occurrence===0&&other.reference.occurrence===1,'Two resources in one HTML event collided')
    const {viewport,root}=container()
    for(let start=0;start<owners[0].document.projectionPages;start+=5){const result=await clients[0].request('projectionWindow',{start,count:Math.min(5,owners[0].document.projectionPages-start)});for(const page of result.pages)root.append(projectionHtml(page.html))}
    let linked
    const references=mountReferences(root,{owner:owners[0],viewport,provider:readerReferenceProvider(clients[0],owners[0]),onLink:value=>{linked=value}})
    views.push(references)
    for(const id of [2,3,4,5,7,9,10]){
      const node=root.querySelector(`[data-image-reference="${id}"]`);check(Boolean(node),'Raw image slot is missing')
      node.scrollIntoView();await wait(()=>node.querySelector('img'))
      check(node.querySelector('img').naturalWidth===2,'Local, inline, table or root/backslash image failed')
    }
    check(!root.querySelector('[onerror],[onclick],[srcset],script')&&globalThis.bad===undefined,'Unsafe raw HTML became active')
    check(!root.querySelector('#source-view,[data-image-reference="999"],[data-document-anchor="forged"]'),'Raw markup forged a host/source identity')
    root.querySelector('[data-link-reference="6"]').click();await wait(()=>linked)
    check(linked.precision==='approximate'&&linked.localPath?.endsWith('next.md'),'Raw local link lost its authority or precision')
    await clients[0].request('navigateFragment',{fragment:'custom-heading'})
    await clients[0].request('navigateFragment',{fragment:'custom-anchor'})
    check(await clients[0].request('navigateFragment',{fragment:'absent-anchor'}).then(()=>false,()=>true),'Unknown fragment was silently accepted')
    const outline=await clients[0].request('outline')
    check(outline.total===3&&outline.items.every(item=>item.label.includes('重复')),'Duplicate Unicode headings lost outline identity')
    check(new Set(outline.items.map(item=>item.id)).size===3,'Duplicate headings reused an identity')
    await clients[0].request('navigate',{location:{kind:'cell',table:0,row:0,column:0}})
    const cell=await clients[0].request('selection')
    check(cell?.location?.kind==='cell'&&cell.precision==='exact'&&cell.quote==='[image: cell]','Image controls changed the original table-cell quote: '+JSON.stringify(cell))

    check(owners[1].capabilities.assetProfile.imageResolution==='source','Source-resolution capability missing')
    const imageHost=container(), image=mountImageView(imageHost.root,{...(await readerImageProvider(clients[1],owners[1],0)),viewport:imageHost.viewport})
    views.push(image);await image.ready
    const bitmap=image.root.querySelector('img')
    check(bitmap.naturalWidth===4096&&bitmap.naturalHeight===1024,'Source image was reduced to a preview')
    check(pixels(bitmap).join(',')==='0,0,0,255,255,255,255,255','Single-pixel source detail was blurred')
    check(bitmap.getBoundingClientRect().width<=600,'Fit image exceeds its viewport')
    image.root.querySelectorAll('button')[3].click()
    check(Math.abs(bitmap.getBoundingClientRect().width-4096)<1,'100% did not use original pixels')
    image.dispose()
    const markdownHost=container(),projection=await clients[2].request('projectionWindow',{start:0,count:Math.min(5,owners[2].document.projectionPages)})
    for(const page of projection.pages)markdownHost.root.append(projectionHtml(page.html))
    const quality=mountReferences(markdownHost.root,{owner:owners[2],viewport:markdownHost.viewport,provider:readerReferenceProvider(clients[2],owners[2])})
    views.push(quality)
    for(const id of [0,1]){
      const node=markdownHost.root.querySelector(`[data-image-reference="${id}"]`);node.scrollIntoView();await wait(()=>node.querySelector('img'))
      check(node.querySelector('img').naturalWidth===4096&&pixels(node.querySelector('img')).join(',')==='0,0,0,255,255,255,255,255','Markdown/raw image lost source pixels')
    }
    for(const view of views)view.dispose()
    check(!containers.some(node=>node.querySelector('img')),'Closing retained image nodes')
    for(const client of clients)await client.dispose()
    return{assertions,rawHtml:true,explicitAnchors:true,sourceResolution:true,approximateRawReferences:true}
  }finally{
    for(const view of views)view.dispose()
    for(const client of clients)await client.dispose({force:true}).catch(()=>{})
    for(const node of [...frames,...containers])node.remove()
  }
}
