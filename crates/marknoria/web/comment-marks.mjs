// Dashed comment anchors use an independent, owned layer. Never wrap text:
// EPUB CFIs, native selections, search ranges and host DOM stay unchanged.
import { uiLabels } from './translation.mjs'
import { commentScopes, commentScopeKey, commentTextMap, commentTextHash, commentRangeAt } from './comment-ranges.mjs'

export const COMMENT_MARK_PROFILE = Object.freeze({ version: 1, scopes: 512, records: 512, bytes: 2 * 1024 * 1024, rectangles: 2048 })
export function mountCommentMarks({ owner, root, viewport = root, load = null, context, open = () => {}, translate = value => value, onStatus = () => {} }) {
  if (typeof owner?.revision !== 'string' || !owner.revision.length || owner.revision.length>256 || typeof owner.documentId !== 'string' || !owner.documentId.length || owner.documentId.length>256 || !root || (load !== null && typeof load !== 'function')) throw TypeError('Comment marks require an owner and scoped provider')
  const document = root.ownerDocument, window = document.defaultView
  const name = 'marknoria-comments-' + crypto.randomUUID().replaceAll('-', '')
  const lifetime = new AbortController(), options = { signal: lifetime.signal }, labels = uiLabels(translate, options)
  let disposed = false, running = false, pending = false, frame = null, timer = null, request = null, generation = 0, fetched = null, records = [], explicit = [], hitboxes = [], hover = null, hideTimer = null
  const hoverRealms = new Map(), modalLayers = new Map(), realms = new Map(), overlay = document.createElement('div'), popup = document.createElement('div')
  overlay.dataset.commentOverlay = ''; overlay.ariaHidden = 'true'
  overlay.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:20;overflow:hidden'
  popup.className = 'comment-mark-popup'; popup.setAttribute('role','group'); labels.set(popup,'Comments',{},'aria-label'); popup.hidden = true
  popup.style.cssText = 'position:fixed;z-index:1500;padding:6px 10px;border:1px solid var(--reader-border,#ccd5d9);border-radius:6px;background:var(--reader-panel,#fff);color:var(--reader-text,#243234);box-shadow:0 3px 12px #0002;cursor:pointer'
  document.body.append(overlay, popup)
  function clearPaint() {
    for (const [realm, style] of realms) { realm.defaultView?.CSS?.highlights?.delete(name); style.remove() }
    realms.clear(); overlay.replaceChildren(); for (const layer of modalLayers.values()) layer.remove(); modalLayers.clear(); hitboxes = []
  }
  const bounds = node => node.getBoundingClientRect()
  function rectangle(rect, clip, frame = null) {
    const frameBox = frame && bounds(frame), wrapper = frame?.parentElement && bounds(frame.parentElement)
    const x = rect.left + (frameBox?.left || 0), y = rect.top + (frameBox?.top || 0), b = bounds(clip || viewport)
    const left = Math.max(x, b.left, wrapper?.left ?? 0, 0), top = Math.max(y, b.top, wrapper?.top ?? 0, 0)
    const right = Math.min(x + rect.width, b.right, wrapper?.right ?? innerWidth, innerWidth), bottom = Math.min(y + rect.height, b.bottom, wrapper?.bottom ?? innerHeight, innerHeight)
    return right > left && bottom > top ? { left, top, right, bottom, width: right - left, height: bottom - top } : null
  }
  async function paint(current, ticket) {
    const shapes = [], maps = new Map(), grouped = new Map(), activeRealms = new Set([document])
    const required = new Set()
    for (const note of records) for (const segment of note.anchor?.textRange?.segments || []) {
      try { required.add(commentScopeKey(segment.scope)) } catch {}
    }
    for (const entry of current.scopes || []) {
      if (!entry.node || !required.has(commentScopeKey(entry.scope))) continue
      try {
        const map = commentTextMap(entry.node)
        const key=commentScopeKey(entry.scope), entries=maps.get(key)||[]
        entries.push({ ...entry, map, hash: await commentTextHash(entry.node, map.text) });maps.set(key,entries)
      } catch {} // Oversized/unrendered scopes fail closed.
    }
    if (ticket !== generation || disposed) return
    for (const note of records) {
      const range = note.anchor?.textRange
      if (range?.version !== 1 || !Array.isArray(range.segments) || range.segments.length > 32) continue
      for (const segment of range.segments) {
        let entries
        try { entries = maps.get(commentScopeKey(segment.scope)) || [] } catch { continue }
        for (const entry of entries) {
          if (entry.hash !== segment.textHash || !entry.node.isConnected) continue
          const resolved = commentRangeAt(entry.map, segment.startUtf16, segment.endUtf16)
          if (resolved) shapes.push({ id: note.id, range: resolved, clip: entry.clip || current.viewport || viewport })
        }
      }
    }
    const extra = await current.resolve?.(records) || []
    if (extra.length > 512) throw Error('Comment mark resolver exceeds its budget')
    shapes.push(...extra)
    if (ticket !== generation || disposed) return
    clearPaint()
    const fragment = document.createDocumentFragment()
    const proposed = getComputedStyle(root).getPropertyValue('--reader-brand').trim()
    const color = proposed.length < 128 && window.CSS.supports('color',proposed) ? proposed : '#287b79'
    let count = 0
    for (const shape of shapes) {
      const rects = shape.range ? shape.range.getClientRects() : shape.rects || []
      if (rects.length > COMMENT_MARK_PROFILE.rectangles - count) throw Error('Comment marks exceed their rectangle budget')
      const realm = shape.range?.startContainer.ownerDocument, view = realm?.defaultView
      if (realm) { activeRealms.add(realm); listenRealm(realm, shape.frame) }
      const native = Boolean(view?.CSS?.highlights && typeof view.Highlight === 'function')
      if (native) { const ranges = grouped.get(realm) || []; ranges.push(shape.range); grouped.set(realm, ranges) }
      for (const rect of rects) {
        count++
        const box = rectangle(rect, shape.clip, shape.frame)
        if (!box) continue
        hitboxes.push({ ...box, id: shape.id, modal:shape.range?.startContainer.parentElement?.closest('dialog[open]') || shape.frame?.closest('dialog[open]') })
        if (native) continue
        const mark = document.createElement('i')
        mark.dataset.commentRect = shape.id
        mark.style.cssText = `position:absolute;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;box-sizing:border-box;border-bottom:1px dashed ${color};pointer-events:none`
        const modal=shape.range?.startContainer.parentElement?.closest('dialog[open]')
        if(modal){
          let layer=modalLayers.get(modal)
          if(!layer){layer=overlay.cloneNode(false);modal.append(layer);modalLayers.set(modal,layer)}
          layer.append(mark)
        }else fragment.append(mark)
      }
    }
    for (const [realm, ranges] of grouped) {
      const style = realm.createElement('style'); style.dataset.commentStyle='' 
      // Studio's dashed anchor treatment (MIT); see third-party/README.md.
      style.textContent = `::highlight(${name}){text-decoration-line:underline;text-decoration-style:dashed;text-decoration-color:${color};text-decoration-thickness:1px;text-underline-offset:.24em}`
      realm.head.append(style); realms.set(realm, style)
      realm.defaultView.CSS.highlights.set(name, new realm.defaultView.Highlight(...ranges))
    }
    overlay.replaceChildren(fragment)
    for(const [realm,controller] of hoverRealms)if(!activeRealms.has(realm)){controller.abort();hoverRealms.delete(realm)}
    root.dataset.commentMarks = String(hitboxes.length)
  }
  function checked(value) {
    if (!Array.isArray(value) || value.length > COMMENT_MARK_PROFILE.records || new TextEncoder().encode(JSON.stringify(value)).length > COMMENT_MARK_PROFILE.bytes || value.some(note => !note || note.kind !== 'comment' || note.revision !== owner.revision || typeof note.id !== 'string' || note.id.length > 256 || typeof note.quote !== 'string' || note.quote.length > 32768 || !note.anchor || typeof note.anchor !== 'object' || Array.isArray(note.anchor))) throw Error('Invalid bounded comment mark set')
    const ids=new Set(), number=value=>Number.isSafeInteger(value)&&value>=0
    for(const note of value){
      if(ids.has(note.id)||!note.id.length||new TextEncoder().encode(note.quote).length>32768)throw Error('Invalid comment identity or quote')
      ids.add(note.id)
      const location=note.anchor.location, range=note.anchor.textRange
      if(location?.kind==='source'&&(!number(location.startByte)||!number(location.endByte)||location.endByte<=location.startByte||location.endByte-location.startByte>128*1024))throw Error('Invalid source comment range')
      if(location?.kind==='pdf'&&(!Array.isArray(location.pages)||!location.pages.length||location.pages.length>64||location.pages.reduce((count,page)=>count+(page.rects?.length||0),0)>256||location.pages.some(page=>!number(page.page)||page.page<1||page.page>2000||!Array.isArray(page.rects)||page.rects.some(rect=>![rect.x,rect.y,rect.width,rect.height].every(Number.isFinite)||rect.x<0||rect.y<0||rect.width<=0||rect.height<=0||rect.x+rect.width>1.001||rect.y+rect.height>1.001))))throw Error('Invalid PDF comment geometry')
      if(location?.kind==='epub'&&(typeof location.cfi!=='string'||location.cfi.length>8192||!location.cfi.startsWith('epubcfi(')||!location.cfi.endsWith(')')||location.href!=null&&(typeof location.href!=='string'||location.href.length>8192)))throw Error('Invalid EPUB comment reference')
      if(range!=null){
        if(range.version!==1||!Array.isArray(range.segments)||!range.segments.length||range.segments.length>32)throw Error('Invalid rendered comment selector')
        const scopes=new Set()
        for(const segment of range.segments){
          const scope=segment.scope
          if(!number(segment.startUtf16)||!number(segment.endUtf16)||segment.endUtf16<=segment.startUtf16||segment.endUtf16>262144||!/^[a-f0-9]{64}$/.test(segment.textHash||''))throw Error('Invalid rendered comment endpoints')
          if(!scope||!['projection','cell','queryCell'].includes(scope.kind)||scope.kind==='projection'&&!number(scope.page)||scope.kind!=='projection'&&(!number(scope.row)||!number(scope.column))||scope.kind==='cell'&&!number(scope.table)||scope.kind==='queryCell'&&(typeof scope.queryId!=='string'||!/^[a-zA-Z0-9_-]{1,128}$/.test(scope.queryId)))throw Error('Invalid rendered comment scope')
          const key=commentScopeKey(scope);if(scopes.has(key))throw Error('Duplicate rendered comment scope');scopes.add(key)
        }
      }
    }
    return structuredClone(value)
  }
  async function update(force = false) {
    if (running) { pending = true; return }
    running = true; pending = false
    const ticket = ++generation
    request?.abort(); request = new AbortController()
    try {
      const current = context?.() || { scopes: commentScopes(root, { viewport }), viewport }
      const keys = [...new Set((current.scopes || []).map(entry => entry.key || commentScopeKey(entry.scope)))].sort()
      if (keys.length > 512) throw Error('Comment view exceeds its scope budget')
      const signature = JSON.stringify(keys)
      if (load && (force || fetched !== signature)) {
        const next = new Map()
        for (let start = 0; start < keys.length; start += 128) {
          let offset = 0, complete = false
          while (!complete) {
            const value = await load({ operation: 'marks', revision: owner.revision, scopes: keys.slice(start, start + 128), offset }, { signal: request.signal })
            request.signal.throwIfAborted()
            if (ticket !== generation || disposed) return
            if (value?.revision !== owner.revision || !Array.isArray(value.items) || value.items.length > 64 || !Number.isSafeInteger(value.nextOffset) || value.nextOffset < offset || (!value.complete && value.nextOffset === offset)) throw Error('Invalid scoped comment response')
            for (const note of checked(value.items)) next.set(note.id, note)
            if (next.size > 512) throw Error('Comment view exceeds 512 records; narrow the visible area')
            offset = value.nextOffset; complete = value.complete === true
          }
        }
        records = checked([...next.values()]); fetched = signature
      } else if (!load) records = explicit
      if (ticket !== generation || disposed || pending) return
      await paint(current, ticket)
      if (ticket === generation) onStatus(null)
    } catch (error) {
      if (ticket !== generation || disposed || error.name === 'AbortError') return
      clearPaint(); currentSourceClear(); delete root.dataset.commentMarks
      onStatus(String(error.message).slice(0, 300))
    } finally {
      running = false
      if (pending && !disposed) refresh()
    }
  }
  const currentSourceClear = () => context?.()?.clear?.()
  function refresh(force = false) {
    if (disposed) return
    if (force) { ++generation; request?.abort(); fetched = null; pending = true; clearHover() }
    if (frame === null) frame = requestAnimationFrame(() => { frame = null; void update() })
  }
  const owned=node=>node.nodeType===1&&(node.matches('[data-comment-overlay],.comment-mark-popup,[data-comment-style]')||node.closest('[data-comment-overlay],.comment-mark-popup'))
  const observer = new MutationObserver(mutations => {
    if(mutations.some(mutation=>!owned(mutation.target)&&(mutation.type!=='childList'||[...mutation.addedNodes,...mutation.removedNodes].some(node=>!owned(node)))))refresh()
  })
  observer.observe(root, { childList: true, characterData: true, subtree: true, attributes: true, attributeFilter: ['hidden','data-ready','data-state','style'] })
  const dialogs = new MutationObserver(() => refresh())
  dialogs.observe(document.body, { childList: true })
  document.addEventListener('scroll', () => { clearHover(); refresh() }, { ...options, capture: true, passive: true })
  window.addEventListener('resize', () => refresh(), options)
  const theme = new MutationObserver(() => refresh())
  theme.observe(document.documentElement, { attributes:true,attributeFilter:['data-theme','style','class'] })
  theme.observe(root, { attributes:true,attributeFilter:['style','class'] })
  root.addEventListener('marknoria:projectionchange', () => refresh(), options)
  function clearHover() { clearTimeout(hover); clearTimeout(hideTimer); popup.hidden=true }
  function pointerMove(event, frame = null) {
    clearTimeout(hideTimer)
    if (popup.contains(event.target)) return
    clearTimeout(hover)
    if (event.buttons || (event.target.ownerDocument || event.currentTarget || document).getSelection?.()?.toString() || document.getSelection()?.toString()) { clearHover(); return }
    const frameBox=frame?.getBoundingClientRect(),x=event.clientX+(frameBox?.left||0),y=event.clientY+(frameBox?.top||0)
    const found=hitboxes.filter(box=>x>=box.left&&x<=box.right&&y>=box.top&&y<=box.bottom)
    if (!found.length) { hideTimer=setTimeout(()=>{if(!popup.contains(document.activeElement))popup.hidden=true},250);return }
    hover = setTimeout(() => {
      if (disposed) return
      const ids = [...new Set(found.map(box=>box.id))], box=found[0]
      const parent=box.modal||document.body
      if(popup.parentElement!==parent)parent.append(popup)
      for (const button of popup.querySelectorAll('button')) labels.clear(button)
      popup.replaceChildren()
      for (const [index,id] of ids.slice(0,16).entries()) {
        const button=document.createElement('button');button.type='button';button.style.cssText='display:block;width:100%;padding:4px 8px;border:0;border-radius:4px;background:transparent;color:inherit;font:inherit;text-align:left;cursor:pointer'
        labels.set(button,ids.length===1?'View comment':'Comment {current} / {total}',{current:index+1,total:ids.length})
        button.addEventListener('click',()=>{clearHover();open(id)}, {once:true})
        popup.append(button)
      }
      if(ids.length>16){const button=document.createElement('button');button.type='button';button.style.cssText='display:block;width:100%;padding:4px 8px;border:0;border-radius:4px;background:transparent;color:inherit;font:inherit;text-align:left;cursor:pointer';labels.set(button,'View all comments');button.addEventListener('click',()=>{clearHover();open(null)});popup.append(button)}
      popup.style.left = `${Math.max(8, Math.min(innerWidth - 200, box.left))}px`
      popup.style.top = `${Math.max(8, Math.min(innerHeight - Math.min(400,popup.scrollHeight||40), box.bottom + 4))}px`
      popup.style.maxHeight='min(400px,80vh)';popup.style.overflow='auto';popup.style.display='';popup.hidden=false
    },400)
  }
  function listenRealm(realm, frame = null) {
    if(hoverRealms.has(realm))return
    const controller=new AbortController();hoverRealms.set(realm,controller)
    realm.fonts?.addEventListener('loadingdone',()=>refresh(),{signal:controller.signal})
    realm.addEventListener('pointermove',event=>pointerMove(event,frame),{signal:controller.signal})
    realm.addEventListener('pointerdown',event=>{if(!popup.contains(event.target))clearHover()},{signal:controller.signal})
    if(realm!==document)realm.addEventListener('scroll',()=>{clearHover();refresh()},{signal:controller.signal,capture:true,passive:true})
  }
  listenRealm(document)
  refresh()
  return {
    name, refresh,
    replace(marks) { if (load) throw Error('Local comment marks are storage-owned'); explicit = checked(marks); clearHover(); refresh() },
    state: () => ({ records: records.length, rectangles: hitboxes.length, disposed }),
    dispose() {
      if (disposed) return
      disposed = true; ++generation; request?.abort(); lifetime.abort(); observer.disconnect(); dialogs.disconnect(); theme.disconnect()
      clearTimeout(timer); clearHover(); for(const controller of hoverRealms.values())controller.abort();hoverRealms.clear(); if (frame !== null) cancelAnimationFrame(frame)
      clearPaint(); currentSourceClear(); overlay.remove(); popup.remove(); delete root.dataset.commentMarks
      records = []; explicit = []
    }
  }
}
