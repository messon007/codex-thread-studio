import { mountImageView } from './image-view.mjs'

export const REFERENCE_VIEW_PROFILE = Object.freeze({ concurrent: 2, mountedImages: 32, pixels: 16777216, deadlineMs: 75000 })

// URI parsing, directory authority, decoding and snapshots belong to Rust.
// This component owns visibility, bounded image mounts and explicit link actions.
export function mountReferences(root, { owner, viewport, provider, onLink, onChange = () => {}, translate = value => value }) {
  if (!root || !viewport || !provider || typeof provider.imageInfo !== 'function' || typeof provider.imageFrame !== 'function' || typeof provider.info !== 'function' || !owner?.documentId || !/^[a-f0-9]{64}$/i.test(owner.revision))
    throw TypeError('References require an owning document, viewport and scoped API')
  const lifetime = new AbortController(), records = new Map(), visible = new Set()
  let stopped = false, active = 0, pixels = 0, mounted = 0, scheduled = null
  const identity = { documentId: owner.documentId, revision: owner.revision }
  const wait = (signal, milliseconds) => new Promise((resolve, reject) => {
    const abort = () => { clearTimeout(timer); signal.removeEventListener('abort',abort); reject(signal.reason) }
    const timer = setTimeout(() => { signal.removeEventListener('abort',abort); resolve() },milliseconds)
    signal.addEventListener('abort',abort,{once:true})
    if (signal.aborted) abort()
  })
  function release(record) {
    record.abort?.abort()
    record.deferred = false
    if (record.view) { record.view.dispose(); record.view = null }
    if (record.charged) { pixels -= record.pixels; mounted--; record.charged = false }
    record.node.replaceChildren(record.fallback.cloneNode(true))
  }
  async function prepare(record) {
    record.loading = true; active++
    const abort = record.abort = new AbortController(), signal = abort.signal, started = performance.now()
    record.node.dataset.referenceState = 'loading'
    try {
      let metadata
      while (true) {
        signal.throwIfAborted()
        if (performance.now()-started > REFERENCE_VIEW_PROFILE.deadlineMs) throw Error('Referenced image exceeded its preparation deadline')
        try { metadata = await provider.imageInfo({id:record.id,signal}) }
        catch (error) { if (error.status !== 429 && error.code !== 'BUSY') throw error; await wait(signal,100); continue }
        if (metadata.state === 'ready') break
        if (metadata.state !== 'indexing') throw Error('Invalid referenced image state')
        await wait(signal,50)
      }
      signal.throwIfAborted()
      const cost = metadata.asset?.width * metadata.asset?.height
      if (!Number.isSafeInteger(cost) || cost < 1 || cost > REFERENCE_VIEW_PROFILE.pixels) throw Error('Referenced image exceeds its pixel budget')
      if (pixels+cost > REFERENCE_VIEW_PROFILE.pixels || mounted >= REFERENCE_VIEW_PROFILE.mountedImages) {
        record.deferred = true
        record.node.dataset.referenceState = 'waiting'
        record.node.title = translate('Waiting for image render budget')
        return
      }
      if (!record.node.isConnected || !visible.has(record)) return
      record.pixels = cost; record.charged = true; pixels += cost; mounted++
      record.node.style.width = `${metadata.asset.width}px`
      record.node.style.aspectRatio = `${metadata.asset.width} / ${metadata.asset.height}`
      record.node.replaceChildren()
      record.view = mountImageView(record.node, {
        owner: identity, viewport, metadata: { ...metadata.asset, alt: record.alt }, translate, compact: true,
        frame: ({frame,signal}) => provider.imageFrame({id:record.id,frame,imageRevision:metadata.imageRevision,signal}),
        onChange: () => onChange(record.node.closest('article')),
      })
      await record.view.ready
      signal.throwIfAborted()
      record.node.dataset.referenceState = 'ready'
      onChange(record.node.closest('article'))
    } catch (error) {
      if (!signal.aborted && !stopped && record.node.isConnected) {
        if (record.view) { record.view.dispose(); record.view = null }
        if (record.charged) { pixels -= record.pixels; mounted--; record.charged = false }
        record.node.replaceChildren(record.fallback.cloneNode(true))
        record.node.dataset.referenceState = 'error'
        record.node.title = String(error.message).slice(0,800)
        record.failed = true
        onChange(record.node.closest('article'))
      }
    } finally { record.loading = false; active--; pump() }
  }
  function pump() {
    if (stopped) return
    for (const record of visible) {
      if (active >= REFERENCE_VIEW_PROFILE.concurrent) break
      if (!record.loading && !record.view && !record.failed && !record.deferred) void prepare(record)
    }
  }
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const record = records.get(entry.target)
      if (!record) continue
      if (entry.isIntersecting) visible.add(record)
      else {
        visible.delete(record); record.failed = false; release(record)
        for (const waiting of visible) waiting.deferred = false
      }
    }
    pump()
  }, { root: viewport, rootMargin: '250px' })
  function scan() {
    scheduled = null
    for (const [node,record] of records) if (!root.contains(node)) { observer.unobserve(node); visible.delete(record); release(record); records.delete(node) }
    for (const node of root.querySelectorAll('[data-image-reference]')) {
      if (records.has(node)) continue
      const id = Number(node.dataset.imageReference)
      if (!Number.isSafeInteger(id) || id < 0 || id >= 4096) continue
      const fallback = root.ownerDocument.createElement('span')
      fallback.className = 'markdown-reference-alt'
      fallback.textContent = node.textContent || translate('Image')
      let alt = '', altBytes = 0
      const description = fallback.textContent.startsWith('[image: ') && fallback.textContent.endsWith(']') ? fallback.textContent.slice(8,-1) : fallback.textContent
      for (const character of description.slice(0,1024)) {
        const bytes = new TextEncoder().encode(character).length
        if (altBytes+bytes > 1024 || /^[\uD800-\uDBFF]$/.test(character)) break
        alt += character; altBytes += bytes
      }
      const record = { id, node, fallback, alt, view: null, abort: null, loading: false, failed: false, deferred: false, pixels: 0, charged: false }
      records.set(node,record); observer.observe(node)
    }
    pump()
  }
  const containsReference = node => node.nodeType === 1 && (node.matches('[data-image-reference]') || node.querySelector('[data-image-reference]'))
  const mutations = new MutationObserver(changes => {
    if (scheduled != null || stopped) return
    if (changes.some(change => [...change.addedNodes,...change.removedNodes].some(containsReference))) scheduled = requestAnimationFrame(scan)
  })
  mutations.observe(root,{childList:true,subtree:true})
  root.addEventListener('click', async event => {
    const link = event.target.closest('a[href]')
    if (!link || !root.contains(link)) return
    event.preventDefault()
    const hasReference = link.hasAttribute('data-link-reference')
    const id = Number(link.dataset.linkReference)
    if (hasReference && (!Number.isSafeInteger(id) || id < 0 || id >= 4096)) return
    try {
      let result
      if (hasReference) {
        result = await provider.info({id,signal:lifetime.signal})
        if (result.reference?.revision !== identity.revision || result.reference.id !== id || result.reference.kind !== 'link' || new TextEncoder().encode(JSON.stringify(result)).length > 262144) throw Error('Invalid or stale link reference')
        result = {...result,target:result.reference.target,precision:'exact'}
      } else {
        const target = link.getAttribute('href') || ''
        if (new TextEncoder().encode(target).length > 8192 || !/^(https?:|mailto:|#)/i.test(target)) throw Error('Unsupported rendered link')
        result = {reference:null,target,localPath:null,openPath:null,precision:'approximate'}
      }
      lifetime.signal.throwIfAborted()
      await onLink?.({ ...identity, ...result })
    } catch (error) { if (!stopped && error.name !== 'AbortError') link.title = String(error.message).slice(0,800) }
  },{signal:lifetime.signal})
  scan()
  return {
    state: () => ({ mounted, pixels, loading: active, waiting: [...visible].filter(record=>record.deferred).length }),
    dispose() {
      if (stopped) return
      stopped = true; lifetime.abort(); mutations.disconnect(); observer.disconnect(); cancelAnimationFrame(scheduled)
      for (const record of records.values()) release(record)
      records.clear(); visible.clear()
    },
  }
}

export function readerReferenceProvider(client, owner) {
  const identity = { documentId: owner.documentId, revision: owner.revision }
  return {
    info: ({id,signal}) => client.request('referenceInfo',{...identity,id},{signal}),
    imageInfo: ({id,signal}) => client.request('referenceImageInfo',{...identity,id},{signal}),
    imageFrame: ({id,imageRevision,frame,signal}) => client.request('referenceImageFrame',{...identity,id,imageRevision,frame},{signal}).then(result=>result.blob),
  }
}
