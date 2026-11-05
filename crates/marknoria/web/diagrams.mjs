const MAX_SVG = 2 * 1024 * 1024
const CACHE_LIMIT = 8 * 1024 * 1024
const cache = new Map()
let cacheBytes = 0
const browserDiagrams = new URLSearchParams(location.search).get('diagrams') === 'browser'
export const diagramMetrics = { renders: 0, hits: 0, errors: 0, cacheBytes: 0, renderMs: [], svgElements: 0, mode: browserDiagrams ? 'browser' : 'backend' }

async function result(id, context) {
  const { documentId, revision, token, signal, browserDiagrams } = context
  signal.throwIfAborted()
  const key = JSON.stringify([documentId, revision, browserDiagrams, id])
  if (cache.has(key)) {
    const entry = cache.get(key)
    cache.delete(key)
    cache.set(key, entry)
    diagramMetrics.hits += 1
    return entry
  }
  const started = performance.now()
  let response
  for (let attempt = 0; attempt < 4; attempt += 1) {
    response = await fetch(new URL(`./api/${browserDiagrams ? 'diagram-source' : 'diagram'}?id=${encodeURIComponent(id)}`, import.meta.url), { signal, headers: { 'x-reader-token': token, 'x-reader-document': documentId } })
    if (response.status !== 429 || attempt === 3) break
    await new Promise(resolve => setTimeout(resolve, 50 * 2 ** attempt))
  }
  if (!response.ok) throw new Error((await response.text()).slice(0, 800))
  const payload = await response.json()
  signal.throwIfAborted()
  if (payload.id !== Number(id) || payload.revision !== revision) throw new Error('Stale diagram response')
  const entry = browserDiagrams
    ? { ...await (await context.engine()).render(payload.kind, payload.source, `reader-diagram-${crypto.randomUUID()}`), renderMs: performance.now() - started, cached: false }
    : payload.result
  signal.throwIfAborted()
  if (typeof entry.svg !== 'string' || entry.svg.length * 2 > MAX_SVG || entry.elements > 20_000) throw new Error('Diagram exceeds its SVG budget')
  entry.bytes = entry.svg.length * 2
  const previous = cache.get(key)
  if (previous) cacheBytes -= previous.bytes
  cache.set(key, entry)
  cacheBytes += entry.bytes
  while (cacheBytes > CACHE_LIMIT) {
    const [oldest, value] = cache.entries().next().value
    cache.delete(oldest)
    cacheBytes -= value.bytes
  }
  diagramMetrics.renders += 1
  diagramMetrics.svgElements = Math.max(diagramMetrics.svgElements, entry.elements)
  diagramMetrics.cacheBytes = cacheBytes
  diagramMetrics.renderMs.push(performance.now() - started)
  if (diagramMetrics.renderMs.length > 100) diagramMetrics.renderMs.shift()
  return entry
}

export function diagramController(viewport, resized, documentContext) {
  if (!documentContext?.documentId || !documentContext?.revision) throw new Error('A diagram requires its owning document and revision')
  const abort = new AbortController()
  let engine
  async function ownedEngine() {
    const { createDiagramRenderer } = await import('./engine.mjs')
    abort.signal.throwIfAborted()
    return engine ||= createDiagramRenderer()
  }
  // Freeze the owner at mount time; never read a later document's diagnostic state.
  const context = Object.freeze({ ...documentContext, signal: abort.signal, engine: ownedEngine, browserDiagrams: documentContext.browserDiagrams ?? browserDiagrams })
  const tracked = new Map()
  const pending = new Set()
  let working = false
  let disposed = false
  async function drain() {
    if (working || disposed) return
    working = true
    try {
      while (pending.size && !disposed) {
        await new Promise(resolve => requestAnimationFrame(resolve))
        const target = pending.values().next().value
        if (!target) continue
        pending.delete(target)
        if (!target.isConnected) continue
        const bounds = target.getBoundingClientRect()
        const visible = viewport.getBoundingClientRect()
        if (bounds.bottom < visible.top - 300 || bounds.top > visible.bottom + 300) {
          target.dataset.diagramState = 'pending'
          observer.observe(target)
          continue
        }
        const owner = tracked.get(target)
        if (!owner) continue
        const canvas = owner.canvas
        target.dataset.diagramState = 'rendering'
        try {
          const entry = await result(target.dataset.diagramId, context)
          if (!target.isConnected || disposed || tracked.get(target) !== owner) continue
          const image = document.createElement('img')
          image.alt = `${target.dataset.diagramKind} diagram`
          const url = URL.createObjectURL(new Blob([entry.svg], { type: 'image/svg+xml' }))
          owner.url = url
          await new Promise((resolve, reject) => {
            const finish = error => {
              image.onload = image.onerror = null
              abort.signal.removeEventListener('abort', cancelled)
              error ? reject(error) : resolve()
            }
            const cancelled = () => finish(new DOMException('Diagram disposed', 'AbortError'))
            abort.signal.addEventListener('abort', cancelled, { once: true })
            image.onload = () => finish()
            image.onerror = () => finish(new Error('Browser could not decode the diagram SVG'))
            image.src = url
          })
          if (!target.isConnected || disposed || tracked.get(target) !== owner) continue
          canvas.replaceChildren(image)
          target.dataset.diagramState = 'rendered'
          target.dataset.svgElements = String(entry.elements)
        } catch (error) {
          if (!target.isConnected || disposed || tracked.get(target) !== owner) continue
          const entry = tracked.get(target)
          if (entry?.url) { URL.revokeObjectURL(entry.url); entry.url = null }
          target.dataset.diagramState = 'error'
          canvas.textContent = `Diagram unavailable: ${error.message}`
          target.querySelector('pre').hidden = false
          target.querySelector('button').setAttribute('aria-expanded', 'true')
          diagramMetrics.errors += 1
        } finally {
          if (target.isConnected && !disposed) resized(target.closest('article'))
        }
      }
    } finally { working = false }
  }
  const observer = new IntersectionObserver(entries => {
    for (const { target, isIntersecting } of entries) {
      if (!isIntersecting || !tracked.has(target) || target.dataset.diagramState !== 'pending') continue
      target.dataset.diagramState = 'queued'
      observer.unobserve(target)
      pending.add(target)
    }
    void drain()
  }, { root: viewport, rootMargin: '300px 0px' })
  function release(figure, entry) {
    if (entry.url) URL.revokeObjectURL(entry.url)
    entry.button.removeEventListener('click', entry.onSource); entry.zoom.removeEventListener('click', entry.onZoom)
    entry.canvas.remove(); entry.button.remove(); entry.zoom.remove()
    entry.pre.hidden = entry.previousHidden
    if (entry.previousState == null) delete figure.dataset.diagramState
    else figure.dataset.diagramState = entry.previousState
    delete figure.dataset.svgElements
  }
  return {
    mount(article) {
      for (const figure of article.querySelectorAll('figure[data-diagram-kind]')) {
        if (tracked.has(figure)) continue
        const pre = figure.querySelector('pre')
        const previousHidden = pre.hidden, previousState = figure.dataset.diagramState
        pre.hidden = true
        const button = document.createElement('button')
        button.textContent = 'Source code'
        button.setAttribute('aria-expanded', 'false')
        const onSource = () => {
          pre.hidden = !pre.hidden
          button.setAttribute('aria-expanded', String(!pre.hidden))
          resized(article)
        }
        button.addEventListener('click', onSource)
        const canvas = document.createElement('div')
        canvas.className = 'diagram-canvas'
        canvas.textContent = 'Rendering nearby diagram…'
        const zoom = document.createElement('button')
        zoom.textContent = 'Actual size'
        zoom.setAttribute('aria-pressed', 'false')
        const onZoom = () => {
          const natural = canvas.classList.toggle('natural')
          zoom.textContent = natural ? 'Fit width' : 'Actual size'
          zoom.setAttribute('aria-pressed', String(natural))
          resized(article)
        }
        zoom.addEventListener('click', onZoom)
        figure.prepend(button, zoom, canvas)
        figure.dataset.diagramState = 'pending'
        tracked.set(figure, { url: null, canvas, button, zoom, pre, previousHidden, previousState, onSource, onZoom })
        observer.observe(figure)
      }
    },
    prune() {
      for (const [figure, entry] of tracked) {
        if (figure.isConnected) continue
        observer.unobserve(figure)
        pending.delete(figure)
        release(figure, entry)
        tracked.delete(figure)
      }
    },
    dispose() {
      disposed = true
      abort.abort()
      engine?.dispose(); engine = null
      pending.clear()
      observer.disconnect()
      for (const [figure, entry] of tracked) release(figure, entry)
      tracked.clear()
    },
    get tracked() { return tracked.size },
  }
}
