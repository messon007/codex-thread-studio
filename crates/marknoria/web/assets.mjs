// Rendering and input only; Rust owns decoding, frame budgets and PDF rasterization.
export function assetController(viewport,index,documentId,changed){
  const nodes=new Map();let disposed=false,documentZoom=null
  function zoomNode(node,state,zoom){state.zoom=zoom;node.classList.toggle("asset-zoomed",zoom!=null);if(zoom==null)node.style.removeProperty("--asset-zoom");else node.style.setProperty("--asset-zoom",zoom)}
  function release(node){const state=nodes.get(node);if(!state)return;state.abort.abort();clearTimeout(state.timer);if(state.url)URL.revokeObjectURL(state.url);observer.unobserve(node);nodes.delete(node)}
  async function paint(node,frame=0){const state=nodes.get(node);if(!state||disposed||!node.isConnected)return
    const asset=index.assets[Number(node.dataset.assetId)];if(!asset)return
    try{let response;for(let attempt=0;attempt<8;attempt++){response=await fetch(new URL(`./api/asset?id=${node.dataset.assetId}&frame=${frame}`,import.meta.url),{signal:state.abort.signal,headers:{'x-reader-token':location.hash.slice(1),'x-reader-document':documentId}});if(response.status!==429)break;await new Promise(r=>setTimeout(r,50*(attempt+1)))}
      if(!response.ok)throw Error((await response.text()).slice(0,800));if(response.headers.get('x-reader-revision')!==index.revision)throw Error('Image revision changed')
      const bytes=await response.blob();if(bytes.size>20*1024*1024)throw Error('Image exceeds its byte budget');if(!node.isConnected||disposed)return
      const url=URL.createObjectURL(bytes);const image=state.image||document.createElement('img');image.alt=asset.alt;image.draggable=false
      await new Promise((resolve,reject)=>{image.onload=resolve;image.onerror=()=>reject(Error('Could not decode image'));image.src=url})
      if(disposed||!node.isConnected||state.abort.signal.aborted){URL.revokeObjectURL(url);return}
      if(state.url)URL.revokeObjectURL(state.url);state.url=url;state.image=image
      node.style.setProperty('--asset-width',`${image.naturalWidth}px`)
      node.querySelector('.document-image').replaceChildren(image);node.dataset.assetState='ready';changed(node.closest('article'))
      if(asset.frames.length>1&&!state.paused){const next=(frame+1)%asset.frames.length;if(next===0)state.plays++;if(!asset.plays||state.plays<asset.plays){state.timer=setTimeout(()=>{const b=node.getBoundingClientRect(),v=viewport.getBoundingClientRect();if(b.bottom<v.top||b.top>v.bottom){observer.observe(node);return}paint(node,next)},Math.max(20,asset.frames[frame].delayMs))}}
    }catch(error){if(error.name!=='AbortError'&&!disposed){node.dataset.assetState='error';node.querySelector('.document-image').textContent=error.message}}
  }
  const observer=new IntersectionObserver(entries=>{for(const entry of entries)if(entry.isIntersecting){observer.unobserve(entry.target);paint(entry.target)}},{root:viewport,rootMargin:'200px'})
  return{mount(article){for(const node of article.querySelectorAll('figure[data-asset-id]')){const asset=index.assets[Number(node.dataset.assetId)];if(!asset||nodes.has(node))continue;const state={abort:new AbortController(),url:null,image:null,timer:null,plays:0,paused:false};nodes.set(node,state)
      const controls=document.createElement('div');controls.className='asset-controls'
      for(const [label,scale]of[['Fit',null],['−',-0.25],['+',0.25],['100%',1]]){const button=document.createElement('button');button.textContent=label;button.addEventListener('click',()=>{const current=state.zoom??(state.image?.getBoundingClientRect().width/state.image?.naturalWidth||1);const zoom=scale==null?null:scale===1?1:Math.min(4,Math.max(.25,current+scale));if(index.format==='pdf'){documentZoom=zoom;for(const [other,value]of nodes){zoomNode(other,value,zoom);changed(other.closest('article'))}}else{zoomNode(node,state,zoom);changed(article)}});controls.append(button)}
      if(asset.frames.length>1){const pause=document.createElement('button');pause.textContent='Pause animation';pause.addEventListener('click',()=>{state.paused=!state.paused;pause.textContent=state.paused?'Play animation':'Pause animation';clearTimeout(state.timer);if(!state.paused){state.plays=0;paint(node)}});controls.append(pause)}
      if(index.format==='pdf')zoomNode(node,state,documentZoom);node.prepend(controls);node.dataset.assetState='pending';observer.observe(node)}},prune(){for(const node of nodes.keys())if(!node.isConnected)release(node)},dispose(){disposed=true;for(const node of nodes.keys())release(node);observer.disconnect()}}
}
