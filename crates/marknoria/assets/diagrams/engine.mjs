import DOMPurify from './vendor/purify.mjs'

const scripts = new Map()
let initialized = false

function script(path) {
  if (!scripts.has(path)) scripts.set(path, new Promise((resolve, reject) => {
    const element = document.createElement('script')
    element.src = new URL(`.${path}`, import.meta.url)
    element.onload = resolve
    element.onerror = () => reject(new Error(`Unable to load ${path}`))
    document.head.append(element)
  }))
  return scripts.get(path)
}

// A controller owns its engine, worker, work deadline and worker URL. The backend
// renderer uses one controller for the lifetime of its isolated browser context.
export function createDiagramRenderer() {
  const lifetime = new AbortController()
  let d2, worker
  const check = () => lifetime.signal.throwIfAborted()
  function cancellable(promise) {
    return new Promise((resolve, reject) => {
      const cancelled = () => reject(lifetime.signal.reason)
      lifetime.signal.addEventListener('abort', cancelled, { once: true })
      promise.then(resolve, reject).finally(() => lifetime.signal.removeEventListener('abort', cancelled))
      if (lifetime.signal.aborted) cancelled()
    })
  }
  function workerFactory(blob) {
    check()
    const url = URL.createObjectURL(blob)
    let released = false
    const release = () => { if (!released) { released = true; URL.revokeObjectURL(url) } }
    try { worker = new Worker(url, { type: 'module' }) } catch (error) { release(); throw error }
    const owned = worker, terminate = owned.terminate.bind(owned)
    const deadline = setTimeout(() => {
      owned.dispatchEvent(new ErrorEvent('error', { message: 'D2 worker initialization exceeded 20 seconds' }))
      owned.terminate()
    }, 20000)
    const loaded = () => { clearTimeout(deadline); release() }
    owned.addEventListener('message', loaded, { once: true })
    owned.addEventListener('error', loaded, { once: true })
    owned.terminate = () => {
      clearTimeout(deadline); release()
      owned.removeEventListener('message', loaded); owned.removeEventListener('error', loaded)
      terminate()
    }
    return owned
  }
  return {
    render: (kind, source, id) => cancellable(renderOwned(kind, source, id, {
      check,
      async d2() {
        const { D2 } = await import('./vendor/d2.js'); check()
        if (!d2) { d2 = new D2({ workerFactory }); d2.ready.catch(() => {}) }
        return d2
      },
    })),
    dispose() {
      if (lifetime.signal.aborted) return
      lifetime.abort()
      // terminate is synchronous even when WASM has not finished initializing.
      worker?.terminate(); worker = null
      void d2?.dispose(); d2 = null
    },
  }
}
let defaultRenderer
export function render(kind, source, id) {
  defaultRenderer ||= createDiagramRenderer()
  return defaultRenderer.render(kind, source, id)
}

async function renderOwned(kind, source, id, owner) {
  owner.check()
  if (source.length > 100_000) throw new Error('Diagram exceeds the 100,000-character source budget')
  let svg
  if (kind === 'mermaid') {
    await script('/vendor/mermaid.min.js')
    owner.check()
    if (!initialized) {
      globalThis.mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', suppressErrorRendering: true, theme: 'default', htmlLabels: false, flowchart: { htmlLabels: false }, maxTextSize: 100_000 })
      initialized = true
    }
    svg = (await globalThis.mermaid.render(id, source)).svg
  } else if (kind === 'plantuml') {
    if (/^\s*!(?:include\w*|import|theme)\b|%\s*load_(?:json|yaml|data)\s*\(/im.test(source)) throw new Error('External PlantUML includes, themes, imports and data loading are disabled')
    await script('/vendor/plantuml-viz-global.js')
    const { renderToString } = await import('./vendor/plantuml.js')
    owner.check()
    const lines = /^\s*@start\w+\b/im.test(source) ? source.trim().split(/\r\n|\r|\n/) : ['@startuml', ...source.split(/\r\n|\r|\n/), '@enduml']
    svg = await new Promise((resolve, reject) => renderToString(lines, resolve, message => reject(new Error(String(message)))))
  } else if (kind === 'd2') {
    const d2 = await owner.d2()
    const result = await d2.compile(source)
    owner.check()
    svg = await d2.render(result.diagram, { ...result.renderOptions, salt: id, noXMLTag: true })
  } else throw new Error('Unknown diagram kind')
  owner.check()
  if (typeof svg !== 'string' || svg.length * 2 > 2 * 1048576) throw new Error('Diagram exceeds the 2 MiB SVG budget')
  const template = document.createElement('template')
  template.innerHTML = DOMPurify.sanitize(svg, { USE_PROFILES: { html: true, svg: true, svgFilters: true }, FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form', 'image'] })
  const root = template.content.querySelector('svg')
  if (!root) throw new Error('Renderer returned no SVG')
  const elements = root.querySelectorAll('*').length
  if (elements > 20_000) throw new Error('Diagram exceeds the 20,000-element SVG budget')
  for (const element of root.querySelectorAll('[href], [xlink\\:href]')) {
    const target = element.getAttribute('href') || element.getAttribute('xlink:href') || ''
    if (!target.startsWith('#')) { element.removeAttribute('href'); element.removeAttribute('xlink:href') }
  }
  if (!root.hasAttribute('xmlns')) root.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  const bounds = root.getAttribute('viewBox')?.trim().split(/[\s,]+/).map(Number)
  if (bounds?.length === 4 && bounds.every(Number.isFinite) && bounds[2] > 0 && bounds[3] > 0) {
    root.setAttribute('width', String(bounds[2]))
    root.setAttribute('height', String(bounds[3]))
  }
  const clean = new XMLSerializer().serializeToString(root)
  if (clean.length * 2 > 2 * 1048576) throw new Error('Sanitized diagram exceeds the 2 MiB SVG budget')
  document.getElementById(id)?.remove()
  document.getElementById(`d${id}`)?.remove()
  return { svg: clean, elements }
}
