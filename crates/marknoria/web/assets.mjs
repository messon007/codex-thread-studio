import { imageFrameResponse, IMAGE_FRAME_BYTES } from './image-frames.mjs'
import { uiLabels } from './translation.mjs'
// Rendering and input only; Rust owns decoding, frame budgets and PDF rasterization.
export function assetController(viewport,index,documentId,changed,options={}){
  if(options.frame!==undefined&&typeof options.frame!=='function')throw TypeError('Image frame provider must be a function')
  const lifetime=new AbortController(),labels=uiLabels(options.translate,{signal:lifetime.signal})
  const nodes=new Map();let disposed=false,documentZoom=null
  function zoomNode(node,state,zoom){state.zoom=zoom;node.classList.toggle("asset-zoomed",zoom!=null);if(zoom==null)node.style.removeProperty("--asset-zoom");else node.style.setProperty("--asset-zoom",zoom)}
  function release(node){const state=nodes.get(node);if(!state)return;state.abort.abort();clearTimeout(state.timer);if(state.url)URL.revokeObjectURL(state.url);observer.unobserve(node);node.querySelector('.document-image')?.replaceChildren();for(const button of node.querySelectorAll('.asset-controls button'))labels.clear(button);node.querySelector('.asset-controls')?.remove();nodes.delete(node)}
  async function paint(node,frame=0){const state=nodes.get(node);if(!state||disposed||!node.isConnected)return
    const asset=index.assets[Number(node.dataset.assetId)];if(!asset)return
    let pendingUrl=null
    try{let bytes;if(options.frame){bytes=await options.frame({id:Number(node.dataset.assetId),frame,signal:state.abort.signal})}else{let response;for(let attempt=0;attempt<8;attempt++){response=await fetch(new URL(`./api/asset?id=${node.dataset.assetId}&frame=${frame}`,import.meta.url),{signal:state.abort.signal,headers:{'x-reader-token':location.hash.slice(1),'x-reader-document':documentId}});if(response.status!==429)break;await new Promise(r=>setTimeout(r,50*(attempt+1)))}
      if(!response.ok)throw Error((await response.text()).slice(0,800));if(response.headers.get('x-reader-revision')!==index.revision)throw Error('Image revision changed')
      bytes=await imageFrameResponse(response,{revision:index.revision,signal:state.abort.signal})}
      if(!(bytes instanceof Blob)||bytes.type!=='image/png'||bytes.size>IMAGE_FRAME_BYTES)throw Error('Expected a bounded PNG frame');if(!node.isConnected||disposed)return
      const url=pendingUrl=URL.createObjectURL(bytes);const image=state.image||document.createElement('img');image.alt=asset.alt;image.draggable=false
      await new Promise((resolve,reject)=>{image.onload=resolve;image.onerror=()=>reject(Error('Could not decode image'));image.src=url})
      if(disposed||!node.isConnected||state.abort.signal.aborted){URL.revokeObjectURL(url);return}
      if(state.url)URL.revokeObjectURL(state.url);state.url=url;pendingUrl=null;state.image=image;state.frame=frame
      node.style.setProperty('--asset-width',`${image.naturalWidth}px`)
      node.querySelector('.document-image').replaceChildren(image);node.dataset.assetState='ready';changed(node.closest('article'));options.onState?.(node)
      if(asset.frames.length>1&&!state.paused){const next=(frame+1)%asset.frames.length;if(next===0)state.plays++;if(!asset.plays||state.plays<asset.plays){state.timer=setTimeout(()=>{const b=node.getBoundingClientRect(),v=viewport.getBoundingClientRect();if(b.bottom<v.top||b.top>v.bottom){observer.observe(node);return}paint(node,next)},Math.min(0x7fffffff,Math.max(20,asset.frames[frame].delayMs)))}}
    }catch(error){if(error.name!=='AbortError'&&!disposed&&!state.abort.signal.aborted&&nodes.get(node)===state&&node.isConnected){node.dataset.assetState='error';node.querySelector('.document-image').textContent=String(error.message).slice(0,800);options.onState?.(node)}}finally{if(pendingUrl)URL.revokeObjectURL(pendingUrl)}
  }
  const observer=new IntersectionObserver(entries=>{for(const entry of entries)if(entry.isIntersecting){observer.unobserve(entry.target);paint(entry.target)}},{root:viewport,rootMargin:'200px'})
  return{mount(article){for(const node of article.matches?.('figure[data-asset-id]')?[article]:article.querySelectorAll('figure[data-asset-id]')){const asset=index.assets[Number(node.dataset.assetId)];if(!asset||nodes.has(node))continue;const state={abort:new AbortController(),url:null,image:null,timer:null,plays:0,paused:false};nodes.set(node,state)
      const controls=document.createElement('div');controls.className='asset-controls'
      for(const [label,scale]of[['Fit',null],['−',-0.25],['+',0.25],['100%',1]]){const button=document.createElement('button');labels.set(button,label);button.type='button';button.addEventListener('click',()=>{const current=state.zoom??(state.image?.getBoundingClientRect().width/state.image?.naturalWidth||1);const zoom=scale==null?null:scale===1?1:Math.min(4,Math.max(.25,current+scale));if(index.format==='pdf'){documentZoom=zoom;for(const [other,value]of nodes){zoomNode(other,value,zoom);changed(other.closest('article'))}}else{zoomNode(node,state,zoom);changed(article)}},{signal:state.abort.signal});controls.append(button)}
      if(asset.frames.length>1){const pause=document.createElement('button');pause.type='button';labels.set(pause,'Pause animation');pause.addEventListener('click',()=>{state.paused=!state.paused;labels.set(pause,state.paused?'Play animation':'Pause animation');clearTimeout(state.timer);if(!state.paused){state.plays=0;paint(node)}},{signal:state.abort.signal});controls.append(pause)}
      if(index.format==='pdf')zoomNode(node,state,documentZoom);node.setAttribute('data-marknoria-image','');node.prepend(controls);node.dataset.assetState='pending';observer.observe(node)}},prune(){for(const node of nodes.keys())if(!node.isConnected)release(node)},state(node){const value=nodes.get(node);return value?{zoom:value.zoom??null,paused:value.paused,frame:value.frame??0,status:node.dataset.assetState}:null},dispose(){disposed=true;lifetime.abort();for(const node of nodes.keys())release(node);observer.disconnect()}}
}
