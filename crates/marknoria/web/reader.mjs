import { createTranslator } from './translation.mjs'
import { readerPresentation } from './presentation.mjs'
import { readerUiLabels } from './reader-ui.mjs'
import { publicReader } from './public-reader.mjs'
import { Virtualizer, observeElementRect, observeElementOffset, elementScroll, defaultRangeExtractor } from './vendor/virtual-core.mjs'
import { projectionHtml } from './projection-html.mjs'
import { diagramController, diagramMetrics } from './diagrams.mjs'
import { assetController } from './assets.mjs'
import { notesController } from './notes.mjs'
import { jqController } from './jq-query.mjs'
import { tableController } from './tables.mjs'
import { fileController } from './files.mjs'
import { sourceController } from './source.mjs'
import { editorController } from './editor.mjs'
import { readPosition, writePosition } from './position.mjs'

const mainLifetime = new AbortController()
const translate = createTranslator()
const on = (target, type, listener, options = {}) => target.addEventListener(type, listener, { ...options, signal: mainLifetime.signal })
const embedded = new URLSearchParams(location.search).has('embedded')
const token = location.hash.slice(1)
let editor = null
let editable = false
let expectedRevision = null
let switching = false
let contentsLoading = false
let readerAbort = null
let stopReader = null
let stopToc = null
const files = fileController(token, { enabled: !embedded, onOpen: () => bridge?.publish('openRequested', publicState()), beforeChange: () => !editor || editor.confirmLeave(), leaving: () => editor?.leaving() })
on(document.querySelector('#compare-files'), 'click', () => {
  if (window.readerExperiment) writePosition(token, documentId, position())
  if (editor && !editor.confirmLeave()) return
  editor?.leaving()
  files.compare()
})
const comparisonPreview = embedded && new URLSearchParams(location.search).has('comparison')
let pendingLine = Number(new URLSearchParams(location.search).get('line')) || 1
on(window, 'message', event => {
  if (!embedded || event.source !== parent || event.origin !== location.origin) return
  if (event.data?.type !== 'marknoria:navigate') return
  pendingLine = Math.max(1, Math.trunc(Number(event.data.line)) || 1)
  if (source) navigateLine(pendingLine)
})
function navigateLine(line, exact = false) {
  if (comparisonPreview || exact) {
    document.querySelector('#preview-mode').click()
    const page = Math.max(0, index.pages.findLastIndex(page => page.line <= line))
    if (index.pages.length) jump(page, null, index.pages[page].rowStart)
    return
  }
  const heading = index.toc.findLast(heading => heading.line <= line)
  if (source.visible) source.go(line)
  else if (heading) jump(heading.page, heading.id)
  else source.show(line)
}
if (embedded) {
  document.querySelector('.reader-shell').classList.add('embedded')
  for (const button of document.querySelectorAll('[data-open-server],#compare-files')) button.hidden = true
}
if (comparisonPreview) document.querySelector('.reader-shell').classList.add('comparison-preview')
let bridge = null
let resourceId = null
let serviceCapabilities = { queries: true, notes: true }
let rendererHeartbeat = null
let queryBusy = 0
let formatRenderer = null, formatReady = null, formatLoading = false
let formatOutlineTotal = null
let documentId = null
function position() {
  if (formatRenderer) { const value = formatRenderer.state(); return { line: 1, view: 'preview', ...(value.format === 'epub' ? { epub: value } : { pdf: { location: value.position?.location, zoom: value.zoom, continuous: value.continuous } }) } }
  if (editor?.visible) return { line: editor.line(), view: 'edit' }
  if (!index || !tables) return { line: 1, view: 'preview' }
  if (source?.visible) return { line: source.currentLine(), view: 'source' }
  const page = readingAnchor()
  const node = nodes.get(tables.itemForPage[page])
  const row = tables.anchor(node) - Number(node?.dataset.rowStart || 0)
  return { line: Number(node?.querySelectorAll('tbody tr')[row]?.dataset.sourceLine) || index.pages[page]?.line || 1, view: 'preview' }
}
const viewport = document.querySelector('#viewport')
const space = document.querySelector('#space')
const toc = document.querySelector('#toc')
const tocSpace = document.querySelector('#toc-space')
const status = document.querySelector('#status')
const shell = document.querySelector('.reader-shell')
const presentation = readerPresentation(shell, { translate, themeRoot: document.documentElement })
readerUiLabels(shell, translate, { signal: mainLifetime.signal })
on(window, 'marknoria:theme', event => { if (['light','dark'].includes(event.detail?.theme)) { presentation.update({ theme: event.detail.theme }); bridge?.publish('state', publicState()) } })
let activeHeading = null
const cache = new Map()
const nodes = new Map()
let diagrams = null
let assets = null
let notes = null
let jq = null
let cacheBytes = 0
let index
let virtualizer
let tocVirtualizer
let tables
let source
let activeRequest = null
let requestedTarget = null
let generation = 0
let frame = null
let mountFrame = null
let stopped = false
let forcedPage = null
let navigation = 0
let documentError = null
let loadError = null
let searchGeneration = 0
const CACHE_LIMIT = 8 * 1024 * 1024
const metrics = { firstContentMs: null, mountedPages: 0, mountedElements: 0, cacheBytes: 0, maxHeartbeatGapMs: 0 }
let started = performance.now()
let heartbeatAt = started
const heartbeat = setInterval(() => {
  const now = performance.now()
  metrics.maxHeartbeatGapMs = Math.max(metrics.maxHeartbeatGapMs, now - heartbeatAt)
  heartbeatAt = now
}, 10)

async function api(path, signal, options = {}) {
  const computing = options.method === 'POST' && ['/api/table-query', '/api/jq-query', '/api/table-chart'].includes(path)
  if (computing) { queryBusy++; bridge?.publish('state', publicState()) }
  try {
    for (let attempt = 0; attempt < 4; attempt += 1) {
      const response = await fetch(new URL(`.${path}`, import.meta.url), { ...options, headers: { ...(options.body ? { 'Content-Type': 'application/json' } : {}), ...options.headers, 'x-reader-token': token, ...(documentId ? { 'x-reader-document': documentId } : {}) }, signal })
      if (response.status === 429 && attempt < 3) {
        await new Promise(resolve => setTimeout(resolve, 50 * 2 ** attempt))
        continue
      }
      if (!response.ok) throw new Error(`Reader request failed (${response.status}): ${(await response.text()).slice(0, 800)}`)
      return await response.json()
    }
  } finally {
    if (computing) { queryBusy--; bridge?.publish('state', publicState()) }
  }
}

function remember(page) {
  if (cache.has(page.id)) cacheBytes -= cache.get(page.id).length * 2
  cache.delete(page.id)
  cache.set(page.id, page.html)
  cacheBytes += page.html.length * 2
  while (cacheBytes > CACHE_LIMIT && cache.size > 5) {
    const [oldest, html] = cache.entries().next().value
    cache.delete(oldest)
    cacheBytes -= html.length * 2
  }
  metrics.cacheBytes = cacheBytes
}

function cached(page) {
  const html = cache.get(page)
  if (html != null) { cache.delete(page); cache.set(page, html) }
  return html
}

function report() {
  bridge?.publish('state', publicState())
  metrics.mountedPages = nodes.size
  metrics.mountedElements = space.querySelectorAll('*').length
  status.textContent = loadError || `${(index.sourceBytes / 1048576).toFixed(2)} MiB · ${index.pages.length} fragments · ${index.toc.length} headings · index ${index.indexMs.toFixed(0)} ms · ${nodes.size} mounted fragments · ${(cacheBytes / 1048576).toFixed(1)} MiB HTML cache`
  if (loadError) document.querySelector('.reader-diagnostics').open = true
}

function schedule() {
  if (frame == null && !stopped) frame = requestAnimationFrame(() => { frame = null; reconcile() })
}

function reconcile() {
  if (!index || stopped || !virtualizer || source?.visible || editor?.visible) return
  const items = virtualizer.getVirtualItems()
  const visible = new Set(items.map(item => item.index))
  for (const [page, node] of nodes) {
    if (!visible.has(page)) { tables.capture(node); node.remove(); nodes.delete(page) }
  }
  diagrams.prune()
  assets?.prune()
  tables.prune()
  virtualizer.measureElement(null)
  space.style.height = `${virtualizer.getTotalSize()}px`
  space.style.width = `${viewport.clientWidth}px`
  space.style.setProperty('--table-viewport-height', `${Math.max(160, viewport.clientHeight - 100)}px`)
  space.style.setProperty('--reading-width', `${Math.max(160, viewport.clientWidth - 48)}px`)
  let mount = null
  let missing = null
  for (const item of items) {
    let node = nodes.get(item.index)
    if (!node) {
      node = document.createElement('article')
      node.className = 'page'
      node.setAttribute('data-marknoria-prose','')
      node.dataset.index = item.index
      const descriptor = tables.items[item.index]
      node.dataset.page = descriptor.page
      node.style.setProperty('--columns', tables.columnCount(descriptor) || 1)
      const content = descriptor.table == null ? cached(descriptor.page) : null
      node.textContent = 'Loading nearby content…'
      node.classList.add('placeholder')
      if (descriptor.table != null) tables.mount(node, descriptor)
      else if (content != null && mount == null) mount = { node, content }
      else if (content == null && missing == null) missing = descriptor.page
      space.append(node)
      nodes.set(item.index, node)
    } else if (tables.items[item.index].table != null) tables.refresh(node)
    else if (!node.dataset.ready && cache.has(tables.items[item.index].page) && mount == null) {
      mount = { node, content: cached(tables.items[item.index].page) }
    } else if (!node.dataset.ready && !cache.has(tables.items[item.index].page) && missing == null) missing = tables.items[item.index].page
    node.style.transform = `translateY(${item.start}px)`
  }
  if (mount && mountFrame == null) {
    mountFrame = requestAnimationFrame(() => {
      mountFrame = null
      if (!mount.node.isConnected || stopped) return
      try { mount.node.replaceChildren(projectionHtml(mount.content)) }
      catch (error) { loadError = error.message; mount.node.textContent = loadError; mount.node.dataset.ready = 'true'; mount.node.classList.remove('placeholder'); report(); return }
      mount.node.dataset.ready = 'true'
      mount.node.classList.remove('placeholder')
      diagrams.mount(mount.node)
      assets?.mount(mount.node)
      virtualizer.measureElement(mount.node)
      if (metrics.firstContentMs == null && !['image','pdf'].includes(index.format)) metrics.firstContentMs = performance.now() - started
      report()
      schedule()
    })
  }
  if (missing != null && !loadError) requestWindow(missing)
  virtualizer._willUpdate()
  tables.position()
  report()
}

async function requestWindow(target) {
  if (requestedTarget != null) return
  requestedTarget = target
  const requestGeneration = generation
  activeRequest = new AbortController()
  try {
    const response = await api(`/api/window?target=${target}`, activeRequest.signal)
    if (requestGeneration !== generation || response.revision !== index.revision || stopped) return
    for (const page of response.pages) remember(page)
  } catch (error) {
    if (error.name !== 'AbortError' && requestGeneration === generation) loadError = error.message
  } finally {
    if (requestGeneration === generation) { requestedTarget = null; activeRequest = null; schedule() }
  }
}

function forcePage(page) {
  forcedPage = page
  virtualizer.setOptions({ ...virtualizer.options, rangeExtractor: range => {
    const pages = defaultRangeExtractor(range)
    if (forcedPage != null && !pages.includes(forcedPage)) pages.push(forcedPage)
    return pages.sort((left, right) => left - right)
  } })
}

function readingAnchor() {
  if (forcedPage != null) return tables.items[forcedPage]?.page ?? 0
  const top = viewport.getBoundingClientRect().top
  const item = [...nodes.entries()].find(([, node]) => {
    const bounds = node.getBoundingClientRect()
    return bounds.top <= top + 1 && bounds.bottom > top + 1
  })?.[0] ?? virtualizer.getVirtualItems().find(item => item.end > viewport.scrollTop)?.index ?? 0
  return tables.items[item]?.page ?? 0
}

async function jump(sourcePage, heading = null, row = null) {
  if (stopped || !index || !tables || !virtualizer || !index.pages[sourcePage]) return
  if (editor?.visible) {
    const target = heading == null ? index.pages[sourcePage]?.line : index.toc.find(item => item.id === heading)?.line
    if (await editor.go(target, index.revision)) {
      activeHeading = heading
      renderToc(tocVirtualizer)
    }
    return
  }
  if (source?.visible) {
    activeHeading = heading ?? index.toc.findLast(item => item.page <= sourcePage)?.id ?? null
    renderToc(tocVirtualizer)
    const line = heading == null ? index.pages[sourcePage].line : index.toc.find(item => item.id === heading)?.line
    await source.go(line)
    return
  }
  const page = tables.target(sourcePage, row)
  activeHeading = heading ?? index.toc.findLast(item => item.page <= sourcePage)?.id ?? null
  renderToc(tocVirtualizer)
  loadError = null
  const ticket = ++navigation
  generation += 1
  activeRequest?.abort()
  requestedTarget = null
  activeRequest = null
  forcePage(page)
  if (tables.items[page].table == null) requestWindow(sourcePage)
  else tables.refresh(nodes.get(page))
  virtualizer.scrollToIndex(page, { align: 'start', behavior: 'auto' })
  schedule()
  let stable = 0
  let previous = null
  for (let attempt = 0; attempt < 150 && ticket === navigation; attempt += 1) {
    await new Promise(resolve => setTimeout(resolve, 20))
    if (stopped || ticket !== navigation || !index || !tables || !virtualizer) return
    const article = nodes.get(page)
    const tableRow = row ?? index.pages[sourcePage]?.rowStart
    const target = heading == null
      ? (tableRow > Number(article?.dataset.rowStart || 0) ? article?.querySelectorAll('tbody tr')[tableRow - Number(article.dataset.rowStart)] : article)
      : document.getElementById(`heading-${heading}`)
    const diagramsReady = article && target && [...article.querySelectorAll('figure[data-diagram-state]')].every(figure => !(figure.compareDocumentPosition(target) & Node.DOCUMENT_POSITION_FOLLOWING) || ['rendered', 'error'].includes(figure.dataset.diagramState))
    if (article?.dataset.ready && target && diagramsReady) {
      virtualizer.cancelScroll()
      const top = target.getBoundingClientRect().top - viewport.getBoundingClientRect().top
      const maximum = viewport.scrollHeight - viewport.clientHeight
      const desired = Math.min(maximum, Math.max(0, viewport.scrollTop + top))
      const aligned = Math.abs(desired - viewport.scrollTop) < 2
      stable = aligned && previous != null && Math.abs(previous - top) < 2 ? stable + 1 : 0
      previous = top
      if (!aligned) viewport.scrollTo({ top: desired, behavior: 'auto' })
      if (stable >= 4) break
    }
  }
  if (ticket === navigation) { forcePage(null); schedule() }
}

function renderToc(instance) {
  if (stopped || !index || !instance || instance !== tocVirtualizer) return
  const fragment = document.createDocumentFragment()
  for (const item of instance.getVirtualItems()) {
    const heading = index.toc[item.index]
    const button = document.createElement('button')
    button.className = 'toc-row'
    button.textContent = heading.label || 'Untitled section'
    button.title = `${heading.label} · line ${heading.line}`
    if (heading.id === activeHeading) button.setAttribute('aria-current', 'location')
    button.style.paddingLeft = `${12 + (heading.level - 1) * 12}px`
    button.style.transform = `translateY(${item.start}px)`
    button.addEventListener('click', () => { setContents(false); jump(heading.page, heading.id) })
    fragment.append(button)
  }
  tocSpace.style.height = `${instance.getTotalSize()}px`
  tocSpace.replaceChildren(fragment)
}

function setup() {
  if(window.readerExperiment){started=performance.now();Object.assign(metrics,{firstContentMs:null,firstAssetMs:null,maxHeartbeatGapMs:0});heartbeatAt=started}
  readerAbort = new AbortController()
  if (['pdfjs','epubjs'].includes(index.renderer)) { setupFormat(); return }
  diagrams = diagramController(viewport, article => { virtualizer?.measureElement(article); schedule() }, { documentId, revision: index.revision, token })
  assets = assetController(viewport,index,documentId,article=>{if(metrics.firstAssetMs==null)metrics.firstAssetMs=performance.now()-started;if(metrics.firstContentMs==null&&['image','pdf'].includes(index.format))metrics.firstContentMs=performance.now()-started;if(article)virtualizer?.measureElement(article);schedule()})
  notes = serviceCapabilities.notes ? notesController(index,api,()=>!editor?.dirty,publicSelection,{documentId,translate}) : null
  jq = serviceCapabilities.queries ? jqController(index,api,{translate}) : null
  files.state('ready')
  document.querySelector('#document-name').textContent = index.name || 'Document'
  document.title = `${index.name || 'Document'} — MarkNoria`
  document.querySelector('.reader-identity p').textContent = `${(index.format||'markdown').toUpperCase()} · Preview`
  document.querySelector('#section-count').textContent = index.toc.length
  document.querySelector('#document-summary').textContent = ['csv','tsv','xlsx'].includes(index.format)
    ? `${((index.originalBytes??index.sourceBytes) / 1048576).toFixed(2)} MiB · ${index.tables.reduce((n,t)=>n+t.rows,0)} rows · ${index.tables.length} tables`
    : `${((index.originalBytes??index.sourceBytes) / 1048576).toFixed(2)} MiB · ${index.toc.length} sections`
  const textFormat=['markdown','csv','tsv','text','html'].includes(index.format||'markdown')
  document.querySelector('#source-mode').hidden=!textFormat
  document.querySelector('#edit-mode').hidden=!textFormat||!editable
  document.querySelector('#compare-files').disabled=!textFormat
  activeHeading = index.toc[0]?.id ?? null
  document.querySelector('#view-modes').hidden = false
  source = sourceController(index, api, line => {
    const heading = index.toc.findLast(heading => heading.line <= line)
    const page = Math.max(0, index.pages.findLastIndex(page => page.line <= line))
    schedule()
    if (index.pages.length) jump(heading?.page ?? page, heading?.id ?? null)
  }, () => {
    const page = readingAnchor()
    const node = nodes.get(tables.itemForPage[page])
    const row = tables.anchor(node) - Number(node?.dataset.rowStart || 0)
    return Number(node?.querySelectorAll('tbody tr')[row]?.dataset.sourceLine) || index.pages[page]?.line || 1
  }, () => {
    tables?.closeOverlays()
    navigation += 1
    generation += 1
    activeRequest?.abort()
    activeRequest = null
    requestedTarget = null
    forcePage(null)
    virtualizer.cancelScroll()
  }, { buttons: !editable, translate })
  tables = tableController(index, api, article => {
    if (article.isConnected) {
      virtualizer?.measureElement(article)
      if (article.dataset.ready && metrics.firstContentMs == null) metrics.firstContentMs = performance.now() - started
    }
    schedule()
  }, { translate, queries: serviceCapabilities.queries, onSelection: () => { notes?.selectionChanged(); document.dispatchEvent(new Event('selectionchange')) } })
  virtualizer = new Virtualizer({
    count: tables.items.length,
    getScrollElement: () => viewport,
    estimateSize: item => tables.items[item].table != null ? 500 : Math.max(60, index.pages[tables.items[item].page].bytes / 3),
    observeElementRect,
    observeElementOffset,
    scrollToFn: elementScroll,
    overscan: 1,
    onChange: schedule,
  })
  virtualizer.shouldAdjustScrollPositionOnItemSizeChange = item => item.end <= viewport.scrollTop
  tocVirtualizer = new Virtualizer({ count: index.toc.length, getScrollElement: () => toc, estimateSize: () => 32, observeElementRect, observeElementOffset, scrollToFn: elementScroll, overscan: 2, onChange: renderToc })
  stopReader = virtualizer._didMount()
  stopToc = tocVirtualizer._didMount()
  virtualizer._willUpdate()
  tocVirtualizer._willUpdate()
  renderToc(tocVirtualizer)
  schedule()
  const cancelNavigation = () => {
    navigation += 1
    forcePage(null)
    virtualizer.cancelScroll()
    schedule()
  }
  viewport.addEventListener('wheel', cancelNavigation, { passive: true, signal: readerAbort.signal })
  viewport.addEventListener('touchstart', cancelNavigation, { passive: true, signal: readerAbort.signal })
  viewport.addEventListener('pointerdown', cancelNavigation, { signal: readerAbort.signal })
  viewport.addEventListener('keydown', event => {
    if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'].includes(event.key)) cancelNavigation()
  }, { signal: readerAbort.signal })
  const changeLayout = update => {
    const anchor = readingAnchor()
    const item = tables.items[tables.itemForPage[anchor]]
    const node = nodes.get(tables.itemForPage[anchor])
    const row = item?.table == null ? null : tables.anchor(node)
    update()
    virtualizer.measure()
    if (item) jump(anchor, null, row)
  }
  document.querySelector('#column-width').addEventListener('change', event => {
    const width = Math.min(600, Math.max(80, Number(event.target.value) || 180))
    event.target.value = width
    changeLayout(() => shell.style.setProperty('--column-width', `${width}px`))
  }, { signal: readerAbort.signal })
  document.querySelector('#wrap').addEventListener('change', event => {
    changeLayout(() => presentation.update({wrap: event.target.checked}))
  }, { signal: readerAbort.signal })
  window.readerExperiment = { index, documentId, metrics, jump, virtualizer, cache, readingAnchor, position, diagrams, diagramMetrics, tables, source, editor, get pendingNavigation() { return forcedPage } }
}

function setupFormat() {
  const owner = { documentId, revision: index.revision, resourceId, signal: readerAbort.signal }
  const pdf = index.renderer === 'pdfjs', label = pdf ? 'PDF' : 'EPUB', saved = readPosition(token, documentId)
  formatLoading = true; space.style.height = pdf ? 'auto' : '100%'; space.style.width = '100%'; files.state('ready')
  document.querySelector('#document-name').textContent = index.name
  document.title = `${index.name} — MarkNoria`
  document.querySelector('.reader-identity p').textContent = `${label} · Preview`
  document.querySelector('#source-mode').hidden = true; document.querySelector('#edit-mode').hidden = true
  document.querySelector('#view-modes').hidden = false; document.querySelector('#compare-files').disabled = true
  document.querySelector('#document-summary').textContent = `${(index.originalBytes / 1048576).toFixed(2)} MiB · Loading ${label}…`
  notes = serviceCapabilities.notes ? notesController(index,api,()=>true,publicSelection,{documentId,translate}) : null
  formatReady = (pdf ? import('./pdf-reader.mjs') : import('./epub-reader.mjs')).then(async module => {
    owner.signal.throwIfAborted()
    const mount = pdf ? module.mountPdfReader : module.mountEpubReader
    formatRenderer = mount(space, { ...owner, viewport, api, translate,
      initialState: saved.epub || { theme: document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light' },
      url: new URL(`./api/original?revision=${owner.revision}`, import.meta.url).href,
      headers: { 'x-reader-token': token, 'x-reader-document': owner.documentId },
      onSelection: value => { bridge?.publish('selection', value); notes?.selectionChanged() },
      onState: value => {
        if (owner.signal.aborted) return
        if (metrics.firstContentMs == null && value.firstUsefulContentMs != null) metrics.firstContentMs = performance.now() - started
        metrics.mountedPages = value.mountedPages ?? space.querySelectorAll('iframe').length
        document.querySelector('#document-summary').textContent = `${(index.originalBytes / 1048576).toFixed(2)} MiB · ${pdf ? `${value.pages} pages` : `${Math.round((value.progress || 0) * 100)}% · ${value.chapterLabel || 'EPUB'}`}`
        status.textContent = value.error || (pdf ? `Page ${value.page} / ${value.pages} · ${value.continuous ? 'Continuous' : 'Paged'}` : `${value.chapterLabel || 'EPUB'} · ${value.flow || 'paginated'} · ${value.progressPrecision || 'chapter'} progress`)
        bridge?.publish('state', publicState())
      },
    })
    await formatRenderer.ready; owner.signal.throwIfAborted()
    if (pdf && saved.pdf) { await formatRenderer.settings({ zoom: saved.pdf.zoom, continuous: saved.pdf.continuous }); if (saved.pdf.location) await formatRenderer.navigate(saved.pdf.location) }
    formatLoading = false; bridge?.publish('state', publicState())
    window.readerExperiment = { index, documentId, metrics, formatRenderer, position }
  }).catch(error => { formatLoading = false; if (!owner.signal.aborted) { documentError = error.message; status.textContent = error.message; files.state('error', error.message) } throw error })
  void formatReady.catch(() => {})
}

async function renderPdfOutline(offset = 0) {
  const owner = formatRenderer, signal = readerAbort.signal
  const outline = await owner.outline({ offset, signal })
  if (signal.aborted || owner !== formatRenderer) return
  formatOutlineTotal = outline.total
  tocSpace.replaceChildren(); tocSpace.style.height = 'auto'
  const action = (label, invoke, disabled = false) => {
    const button = document.createElement('button'); button.className = 'pdf-outline-row'; button.textContent = label; button.disabled = disabled
    button.addEventListener('click', () => { Promise.resolve().then(invoke).catch(error => { status.textContent = error.message }) }, { signal })
    tocSpace.append(button); return button
  }
  for (const item of outline.items) {
    const button = action(item.label, () => { setContents(false); return owner.navigate(item.location) }, !item.location)
    button.style.paddingLeft = `${12 + item.level * 12}px`
  }
  if (offset > 0) action('Previous outline entries', () => renderPdfOutline(Math.max(0, offset - 256)))
  if (offset + outline.items.length < outline.total) action('Next outline entries', () => renderPdfOutline(offset + outline.items.length))
  document.querySelector('#section-count').textContent = outline.total
}

function teardown() {
  searchGeneration += 1
  navigation += 1
  generation += 1
  activeRequest?.abort()
  activeRequest = null
  requestedTarget = null
  readerAbort?.abort()
  formatRenderer?.dispose(); formatRenderer = null; formatReady = null; formatLoading = false
  formatOutlineTotal = null
  if (frame != null) cancelAnimationFrame(frame)
  if (mountFrame != null) cancelAnimationFrame(mountFrame)
  frame = mountFrame = null
  source?.dispose(); source = null
  tables?.dispose(); tables = null
  diagrams?.dispose(); diagrams = null
  assets?.dispose(); assets = null
  notes?.dispose(); notes = null
  jq?.dispose(); jq = null
  stopReader?.(); stopToc?.()
  stopReader = stopToc = null
  virtualizer = tocVirtualizer = null
  nodes.clear(); cache.clear(); cacheBytes = 0
  space.replaceChildren(); tocSpace.replaceChildren()
  forcedPage = null
  loadError = null
  index = null
}

function configureEditor(response) {
  if (response.documentId&&!response.editable){editable=false;document.querySelector('#edit-mode').hidden=true}
  if (!response.editable || !response.documentId || comparisonPreview) return
  editable = true
  documentId = response.documentId
  document.querySelector('#view-modes').hidden = false
  document.querySelector('#edit-mode').hidden = false
  document.querySelector('.reader-footer-hint').textContent = 'Save or change editing pages to write'
  document.querySelector('#welcome small').innerHTML = window.marknoriaDesktop
    ? 'Ctrl / ⌘ O to open · Choose files from any local folder<br>Use Edit to change the file, then Save or turn the page.'
    : 'Ctrl / ⌘ O to open · Files stay on the server<br>Use Edit to change the file, then Save or turn the page.'
  if (editor) return
  editor = editorController(token, {
    translate, nativeNotifications: !embedded,
    documentId: () => documentId,
    saved(next) { documentId = next.documentId; expectedRevision = next.revision; bridge?.publish('saved', { documentId, revision: next.revision, previewRevision: index?.revision, needsRefresh: true }) },
    pause() {
      tables?.closeOverlays()
      navigation += 1
      generation += 1
      activeRequest?.abort()
      requestedTarget = null
      document.querySelector('#workspace').hidden = true
      setContents(false); setSearch(false)
    },
    changed(state) {
      bridge?.publish('state', publicState())
      for (const id of ['preview-mode', 'source-mode', 'edit-mode', 'compare-files']) document.getElementById(id).disabled = state.busy
      for (const button of document.querySelectorAll('[data-open-server]')) button.disabled = state.busy
      document.getElementById('contents-toggle').disabled = state.busy || contentsLoading || !index
      document.getElementById('search-toggle').disabled = state.visible || !index
      document.querySelector('.reader-settings').hidden = state.visible || Boolean(source?.visible)
      notice.hidden = state.visible || !state.dirty
    },
  })
}

const notice = document.createElement('div')
notice.id = 'edit-notice'
notice.hidden = true
notice.textContent = 'This view shows the saved file. Edit * keeps your unsaved page in memory; Save to refresh the reading views.'
shell.append(notice)

async function refreshReading() {
  const targetRevision = expectedRevision
  teardown()
  while (!stopped) {
    const response = await api('/api/document', mainLifetime.signal)
    if (response.state === 'error') throw Error(response.error)
    if (response.state === 'idle') throw Error('Open a document first')
    if (response.state === 'ready') {
      if (targetRevision && response.index.revision !== targetRevision) throw Error('The reading index does not match the saved file. Reopen the file to refresh it.')
      documentId = response.documentId
      resourceId = response.resourceId
      index = response.index
      for (const page of response.initial) remember(page)
      setup()
      if (formatReady) await formatReady
      editor?.refreshed()
      return
    }
    status.textContent = 'Refreshing the saved reading view…'
    await new Promise(resolve => setTimeout(resolve, 100))
  }
}

async function switchView(view) {
  if (!editable || switching || editor?.busy) return
  const current = editor.visible ? 'edit' : source?.visible ? 'source' : 'preview'
  if (view === current && index && !editor.needsRefresh) return
  switching = true
  const line = position().line
  setContents(false)
  try {
    if (view === 'edit') {
      source?.hide()
      await editor.show(line)
    } else {
      if (!index || editor.needsRefresh) await refreshReading()
      editor.hide()
      document.getElementById('workspace').hidden = false
      if (view === 'source') await source.show(line)
      else {
        const fromSource = source.visible
        source.hide()
        schedule()
        if (!fromSource && index.pages.length) {
          const heading = index.toc.findLast(heading => heading.line <= line)
          const page = Math.max(0, index.pages.findLastIndex(page => page.line <= line))
          jump(heading?.page ?? page, heading?.id ?? null)
        }
      }
    }
    for (const mode of ['preview', 'source', 'edit']) document.getElementById(mode + '-mode').setAttribute('aria-pressed', String(mode === view))
    const format = (index.format||'markdown').toUpperCase()
    document.querySelector('.reader-identity p').textContent = view === 'edit' ? `${format} · Paged editing` : view === 'source' ? `${format} · Read-only source` : `${format} · Read-only preview`
  } catch (error) {
    status.textContent = error.message
    document.querySelector('.reader-diagnostics').open = true
  } finally { switching = false }
}
for (const view of ['preview', 'source', 'edit']) on(document.getElementById(view + '-mode'), 'click', () => switchView(view))

on(window, 'pagehide', () => {
  mainLifetime.abort()
  if (!comparisonPreview && index && !documentError && (!formatRenderer || formatRenderer.state().ready)) writePosition(token, documentId, position())
  stopped = true
  teardown()
  editor?.dispose()
  files.dispose()
  clearInterval(heartbeat)
  clearInterval(rendererHeartbeat)
  bridge?.dispose()
}, { once: true })

function setContents(open) {
  document.querySelector('#reader-outline').hidden = !open
  document.querySelector('#contents-backdrop').hidden = !open
  document.querySelector('#contents-toggle').setAttribute('aria-expanded', String(open))
  if (open) { tocVirtualizer?._willUpdate(); if (tocVirtualizer) renderToc(tocVirtualizer) }
  bridge?.publish('state', publicState())
}

function setSearch(open) {
  if (!open) searchGeneration += 1
  document.querySelector('#search').hidden = !open
  document.querySelector('#search-toggle').setAttribute('aria-expanded', String(open))
  document.querySelector('#matches').hidden = !open
  if (open) document.querySelector('#query').focus()
}

async function showContents(open) {
  if (!open) { setContents(false); return }
  if (switching || contentsLoading || editor?.busy) throw Object.assign(Error('Reader is busy'),{code:'BUSY'})
  contentsLoading = true
  switching = true
  document.getElementById('contents-toggle').disabled = true
  try {
    if (editor?.visible && editor.needsRefresh) await refreshReading()
    if (!index || editor?.busy) return
    if (formatRenderer) {
      await renderPdfOutline()
      setContents(true); return
    }
    if (editor?.visible && editor.revision !== index.revision) throw Error('The outline is refreshing. Reopen Contents to try again.')
    if (editor?.visible) activeHeading = index.toc.findLast(heading => heading.line <= editor.line())?.id ?? null
    document.getElementById('reader-outline').title = editor?.visible ? 'Outline of the saved file. Changes in the current editing page are mapped when jumping.' : ''
    setContents(true)
  } catch (error) {
    status.textContent = error.message
    document.querySelector('.reader-diagnostics').open = true
    throw error
  } finally {
    contentsLoading = false
    switching = false
    document.getElementById('contents-toggle').disabled = Boolean(editor?.busy) || !index
    bridge?.publish('state', publicState())
  }
}
on(document.querySelector('#contents-toggle'), 'click', () => { showContents(document.querySelector('#reader-outline').hidden).catch(()=>{}) })
on(document.querySelector('#contents-close'), 'click', () => setContents(false))
on(document.querySelector('#contents-backdrop'), 'click', () => setContents(false))
on(document.querySelector('#search-toggle'), 'click', () => setSearch(document.querySelector('#search').hidden))
on(document.querySelector('#search-close'), 'click', () => setSearch(false))
on(document, 'keydown', event => { if (event.key === 'Escape') { setContents(false); setSearch(false); document.querySelector('.reader-settings').open = false } })
on(document, 'pointerdown', event => {
  const settings = document.querySelector('.reader-settings')
  if (settings.open && !settings.contains(event.target)) settings.open = false
})

on(document.querySelector('#search'), 'submit', async event => {
  event.preventDefault()
  const ticket = ++searchGeneration
  try {
    const query = document.querySelector('#query').value
    const response = formatRenderer ? await formatRenderer.search(query, { signal: mainLifetime.signal }) : await api(`/api/search?query=${encodeURIComponent(query)}`, mainLifetime.signal)
    if (ticket !== searchGeneration) return
    const list = document.querySelector('#matches')
    list.replaceChildren()
    for (const match of response.matches) {
      const item = document.createElement('li')
      const button = document.createElement('button')
      button.textContent = formatRenderer ? `${match.label || `Page ${match.page}`}: ${match.excerpt}` : `${match.line}: ${match.snippet}`
      button.addEventListener('click', () => { list.hidden = true; if (formatRenderer) formatRenderer.navigate(match.location).catch(error => { status.textContent = error.message }); else if (source?.visible) source.go(match.line); else jump(match.page, null, match.tableRow) })
      item.append(button); list.append(item)
    }
    status.textContent = formatRenderer ? `${response.matches.length} matching ${index.format === 'pdf' ? 'pages' : 'passages'}${response.complete ? '' : ' · Result limit reached'}` : `${response.matches.length} matching source lines (maximum ${response.limit}); case-sensitive`
    list.hidden = false
    if (!response.matches.length) {
      const empty = document.createElement('li')
      empty.textContent = 'No matching source lines'
      list.append(empty)
    }
  } catch (error) { if (ticket === searchGeneration) status.textContent = error.message }
})

function publicState() {
  const view = editor?.visible ? 'edit' : source?.visible ? 'source' : 'preview'
  const sourceAvailable = Boolean(index&&['markdown','csv','tsv','text','html'].includes(index.format))
  return { apiVersion: 1, bridgeVersion: 1, state: stopped ? 'disposed' : documentError ? 'error' : index ? formatLoading || formatRenderer?.state().ready === false ? 'rendering' : 'ready' : 'indexing', error: documentError,
    documentId, resourceId, revision: index?.revision ?? null, editingRevision: editor?.revision ?? null,
    presentation: presentation.state(),
    timings: { indexMs: index?.indexMs ?? null, indexPeakRssKiB: index?.peakRssKib ?? null, firstUsefulContentMs: metrics.firstContentMs, maxHeartbeatGapMs: metrics.maxHeartbeatGapMs },
    editorError: editor?.error ? { code: editor.error.code || 'BACKEND', message: editor.error.message } : null,
    name: index?.name ?? null, format: index?.format ?? null, formatState: formatRenderer?.state() ?? null, view,
    document: index ? {fileBytes:index.originalBytes??index.sourceBytes,sourceBytes:sourceAvailable?index.sourceBytes:null,sourceLines:sourceAvailable?index.sourceLines:null,projectionPages:index.pages.length,tables:index.tables.length} : null,
    outline: {open:!document.querySelector('#reader-outline').hidden,available:Boolean(index&&(formatRenderer||index.toc.length)),total:index?.renderer?formatOutlineTotal:index?.toc.length??0},
    dirty: Boolean(editor?.dirty), busy: Boolean(switching || editor?.busy || queryBusy || formatLoading || formatRenderer?.state().busy), needsRefresh: Boolean(editor?.needsRefresh),
    capabilities: { ...serviceCapabilities, projectionWindow: Boolean(index && !formatRenderer), presentation: { version: 1, locale: true, palette: true, fonts: true }, charts: Boolean(serviceCapabilities.charts && index?.tables.length), edit: editable, source: sourceAvailable },
    positionPrecision: editor?.dirty ? 'draft' : formatRenderer?.state().format === 'epub' ? 'exact' : 'approximate',
    position: formatRenderer ? formatRenderer.state().position : index && tables && !editor?.dirty ? { version: 1, resourceId, revision: view === 'edit' ? editor.revision : index.revision, view, location: { kind: 'line', line: position().line } } : null }
}
async function publicSelection(signal) {
  if (!index) return null
  if (formatRenderer) return formatRenderer.selection()
  if (tables?.hasOverlay()) { const value = tables.selection(); return value ? { documentId, revision: index.revision, ...value } : null }
  const code = editor?.visible ? editor.code : source?.visible ? source.code : null
  const range = getSelection()?.rangeCount ? getSelection().getRangeAt(0) : null
  const bounds = range?.getBoundingClientRect()
  const viewportRect = bounds ? { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } : null
  if (code) {
    let selected = code.view.state.selection.main
    if (range && !range.collapsed && code.view.contentDOM.contains(range.startContainer) && code.view.contentDOM.contains(range.endContainer)) {
      const from = code.view.posAtDOM(range.startContainer, range.startOffset), to = code.view.posAtDOM(range.endContainer, range.endOffset)
      selected = { from: Math.min(from, to), to: Math.max(from, to), empty: from === to }
    }
    if (selected.empty) return null
    if (editor?.visible && editor.dirty) {
      const text = code.view.state.sliceDoc(selected.from, selected.to), quote = boundedQuote(text)
      return { documentId, revision: editor.revision, quote, quoteTruncated: quote.length < text.length,
        precision: 'draft', location: { kind: 'line', line: editor.line() }, viewportRect }
    }
    if (editor?.visible && editor.needsRefresh) throw Error('Refresh the saved reading view before capturing an exact source selection')
    const page = editor?.visible ? editor.data.window : source.page
    const selection = await api('/api/selection/source', signal, { method: 'POST', body: JSON.stringify({ revision: index.revision,
      start: page.start, end: page.end, fromUtf16: selected.from, toUtf16: selected.to }) })
    if (stopped || !index || selection.documentId !== documentId || selection.revision !== index.revision) throw Object.assign(Error('Selection belongs to a previous reader revision'), { code: 'REVISION' })
    return { ...selection, viewportRect }
  }
  if (!range || range.collapsed) { const value = tables?.selection(); return value ? { documentId, revision: index.revision, ...value } : null }
  const element = node => node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement
  const start = element(range.startContainer), end = element(range.endContainer)
  const page = start?.closest('.page')
  if (!page || !space.contains(page) || !space.contains(end)) { const value = tables?.selection(); return value ? { documentId, revision: index.revision, ...value } : null }
  const text = range.toString(), quote = boundedQuote(text), cell = start.closest('td,th')
  let location = { kind: 'line', line: Number(start.closest('[data-source-line]')?.dataset.sourceLine) || index.pages[Number(page.dataset.page)]?.line || 1 }, precision = 'approximate'
  if (cell?.tagName === 'TD' && cell === end.closest('td,th') && page.dataset.table != null) {
    const columns = JSON.parse(page.dataset.selectedColumns || '[]')
    location = { kind: 'cell', table: Number(page.dataset.table), row: Number(page.dataset.rowStart) + cell.parentElement.sectionRowIndex, column: columns[cell.cellIndex] }
    precision = 'exact'
  }
  return { documentId, revision: index.revision, quote, quoteTruncated: quote.length < text.length, precision, location, viewportRect }
}
function boundedQuote(text) {
  let end = Math.min(text.length, 16384)
  if (end && /[\uD800-\uDBFF]/.test(text[end - 1])) end--
  return text.slice(0, end)
}
function connectBridge() {
  const lifetime = new AbortController()
  let selectionTimer = null, selectionRequest = null, selectionGeneration = 0
  const result = publicReader({ origins: serviceCapabilities.parentOrigins || [], state: publicState, index: () => index, renderer: () => formatRenderer, api,
    contents: showContents,
    async setView(view) {
      if (!index || editor?.busy || switching) throw Error('Reader is busy')
      if (view === 'edit' && !editable) throw Error('Editing was not granted')
      if (view !== 'preview' && !publicState().capabilities.source) throw Error('Source is unavailable for this format')
      if (editable) await switchView(view)
      else if (view === 'source') await source.show(position().line)
      else source?.hide()
      if (publicState().view !== view) throw Error('Could not switch the reader view')
      result.publish('state', publicState())
    },
    async navigate(location) {
      if (!location || !index) throw Error('A reading location is required')
      if (formatRenderer) { await formatRenderer.navigate(location); result.publish('state', publicState()); return }
      if (location.kind === 'line') {
        if (!Number.isSafeInteger(location.line) || location.line < 1 || location.line > index.sourceLines) throw Error('Source line out of range')
        if (editor?.visible) { if (!await editor.go(location.line)) throw Error('Editor navigation failed') }
        else if (source?.visible) await source.go(location.line)
        else { const page = Math.max(0, index.pages.findLastIndex(page => page.line <= location.line)); await jump(page) }
      } else if (location.kind === 'cell') {
        const table = Number.isSafeInteger(location.table) && location.table >= 0 ? index.tables[location.table] : null
        if (!table || !Number.isSafeInteger(location.row) || location.row < 0 || location.row >= table.rows || !Number.isSafeInteger(location.column) || location.column < 0 || location.column >= table.columns.length) throw Error('Table cell out of range')
        const revision = index.revision
        if (editor?.visible || source?.visible) {
          if (editable) await switchView('preview')
          else source.hide()
          if (index.revision !== revision) throw Object.assign(Error('Document revision changed while opening Preview'), { code: 'REVISION' })
        }
        await jump(table.firstPage, null, location.row)
        await tables.go(location.table, location.row, location.column)
      } else throw Object.assign(Error('This renderer cannot resolve this location yet; retain the original location'), { code: 'UNSUPPORTED_LOCATION' })
      result.publish('state', publicState())
    },
    selection: publicSelection,
    async save() { if (!editable || !editor) throw Object.assign(Error('Editing was not granted'), { code: 'CAPABILITY' }); if (!await editor.save()) throw editor.error || Error('The editing page was not saved') },
    async settings(params) {
      const before = presentation.state()
      presentation.update(params)
      if (params.theme != null) window.dispatchEvent(new CustomEvent('marknoria:theme-request', { detail: { theme: params.theme } }))
      if (params.wrap != null && params.wrap !== before.wrap) { const input = document.getElementById('wrap'); input.checked = params.wrap; input.dispatchEvent(new Event('change')) }
      if (formatRenderer && params.pdf) await formatRenderer.settings(params.pdf)
      if (formatRenderer?.state().format === 'epub' && (params.epub || params.theme)) await formatRenderer.settings({ ...(params.theme ? { theme: params.theme } : {}), ...params.epub })
      schedule()
    },
    async dispose() { mainLifetime.abort(); presentation.dispose(); lifetime.abort(); shell.inert = true; window.readerExperiment = null; clearTimeout(selectionTimer); selectionRequest?.abort(); stopped = true; const closing = formatRenderer?.dispose(); teardown(); editor?.dispose(); files.dispose(); clearInterval(heartbeat); clearInterval(rendererHeartbeat); await closing },
  })
  document.addEventListener('selectionchange', () => {
    if (!result.connected) return
    clearTimeout(selectionTimer); const generation = ++selectionGeneration
    selectionRequest?.abort()
    selectionTimer = setTimeout(async () => {
      selectionRequest = new AbortController()
      try { const value = await publicSelection(selectionRequest.signal); if (generation === selectionGeneration) result.publish('selection', value) } catch {}
    }, 150)
  }, { signal: lifetime.signal })
  return result
}

try {
  if (!token) throw new Error('Open the full URL printed by the reader, including its #token')
  serviceCapabilities = await api('/api/capabilities', mainLifetime.signal)
  if (embedded) bridge = connectBridge()
  while (!index && !stopped) {
    const response = await api('/api/document', mainLifetime.signal)
    configureEditor(response)
    if (response.state === 'idle') { files.state('idle'); clearInterval(heartbeat); break }
    if (response.state === 'error') throw new Error(response.error)
    if (response.state === 'ready') {
      documentId = response.documentId
      resourceId = response.resourceId
      index = response.index
      for (const page of response.initial) remember(page)
      setup()
      if (formatReady) await formatReady
      bridge?.publish('ready', publicState())
      const saved = readPosition(token, documentId)
      if (!new URLSearchParams(location.search).has('line') && pendingLine === 1) {
        if (saved.view === 'edit' && editable) await switchView('edit')
        else if (saved.view === 'source') await source.show(saved.line)
        else if (saved.line > 1) navigateLine(saved.line, true)
      } else if (pendingLine > 1) navigateLine(pendingLine)
      if (stopped) break
      if (comparisonPreview) parent.postMessage({ type: 'marknoria:ready', documentId }, location.origin)
      if (embedded) rendererHeartbeat = setInterval(() => api('/api/renderer').catch(() => {}), 30_000)
      break
    }
    files.state('indexing')
    await new Promise(resolve => setTimeout(resolve, 150))
  }
} catch (error) {
  if (!stopped) {
  documentError = error.message
  bridge?.publish('error', publicState())
  status.textContent = error.message
  files.state('error', error.message)
  document.querySelector('.reader-diagnostics').open = true
  }
  clearInterval(heartbeat)
}
