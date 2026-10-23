import { Virtualizer, observeElementRect, observeElementOffset, elementScroll, defaultRangeExtractor } from './vendor/virtual-core.mjs'
import DOMPurify from './vendor/purify.mjs'
import { diagramController, diagramMetrics } from './diagrams.mjs'
import { tableController } from './tables.mjs'
import { fileController } from './files.mjs'
import { sourceController } from './source.mjs'
import { editorController } from './editor.mjs'
import { readPosition, writePosition } from './position.mjs'

const token = location.hash.slice(1)
let editor = null
let editable = false
let expectedRevision = null
let switching = false
let contentsLoading = false
let readerAbort = null
let stopReader = null
let stopToc = null
const files = fileController(token, { beforeChange: () => !editor || editor.confirmLeave(), leaving: () => editor?.leaving() })
document.querySelector('#compare-files').addEventListener('click', () => {
  if (window.readerExperiment) writePosition(token, documentId, position())
  if (editor && !editor.confirmLeave()) return
  editor?.leaving()
  files.compare()
})
const embedded = new URLSearchParams(location.search).has('embedded')
const comparisonPreview = embedded && new URLSearchParams(location.search).has('comparison')
let pendingLine = Number(new URLSearchParams(location.search).get('line')) || 1
addEventListener('message', event => {
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
let documentId = null
function position() {
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
let activeHeading = null
const cache = new Map()
const nodes = new Map()
let diagrams = null
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
let loadError = null
let searchGeneration = 0
const CACHE_LIMIT = 8 * 1024 * 1024
const metrics = { firstContentMs: null, mountedPages: 0, mountedElements: 0, cacheBytes: 0, maxHeartbeatGapMs: 0 }
const started = performance.now()
let heartbeatAt = started
const heartbeat = setInterval(() => {
  const now = performance.now()
  metrics.maxHeartbeatGapMs = Math.max(metrics.maxHeartbeatGapMs, now - heartbeatAt)
  heartbeatAt = now
}, 10)

async function api(path, signal) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const response = await fetch(new URL(`.${path}`, import.meta.url), { headers: { 'x-reader-token': token, ...(documentId ? { 'x-reader-document': documentId } : {}) }, signal })
    if (response.status === 429 && attempt < 3) {
      await new Promise(resolve => setTimeout(resolve, 50 * 2 ** attempt))
      continue
    }
    if (!response.ok) throw new Error(`Reader request failed (${response.status})`)
    return response.json()
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
  metrics.mountedPages = nodes.size
  metrics.mountedElements = space.querySelectorAll('*').length
  status.textContent = loadError || `${(index.sourceBytes / 1048576).toFixed(2)} MiB · ${index.pages.length} fragments · ${index.toc.length} headings · index ${index.indexMs.toFixed(0)} ms · ${nodes.size} mounted fragments · ${(cacheBytes / 1048576).toFixed(1)} MiB HTML cache`
  if (loadError) document.querySelector('.reader-diagnostics').open = true
}

function schedule() {
  if (frame == null && !stopped) frame = requestAnimationFrame(() => { frame = null; reconcile() })
}

function reconcile() {
  if (!index || stopped || source?.visible || editor?.visible) return
  const items = virtualizer.getVirtualItems()
  const visible = new Set(items.map(item => item.index))
  for (const [page, node] of nodes) {
    if (!visible.has(page)) { tables.capture(node); node.remove(); nodes.delete(page) }
  }
  diagrams.prune()
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
      mount.node.innerHTML = DOMPurify.sanitize(mount.content, { USE_PROFILES: { html: true }, FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'style'] })
      for (const link of mount.node.querySelectorAll('a')) { link.target = '_blank'; link.rel = 'noopener noreferrer' }
      for (const row of mount.node.querySelectorAll('tr[data-source-line]')) row.title = `Source line ${row.dataset.sourceLine}`
      mount.node.dataset.ready = 'true'
      mount.node.classList.remove('placeholder')
      diagrams.mount(mount.node)
      virtualizer.measureElement(mount.node)
      if (metrics.firstContentMs == null) metrics.firstContentMs = performance.now() - started
      report()
      schedule()
    })
  }
  if (missing != null && !loadError) requestWindow(missing)
  virtualizer._willUpdate()
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
    const article = nodes.get(page)
    const target = heading == null ? article : document.getElementById(`heading-${heading}`)
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
  readerAbort = new AbortController()
  diagrams = diagramController(viewport, article => { virtualizer?.measureElement(article); schedule() })
  files.state('ready')
  document.querySelector('#document-name').textContent = index.name || 'Document'
  document.title = `${index.name || 'Document'} — MarkNoria`
  document.querySelector('#section-count').textContent = index.toc.length
  document.querySelector('#document-summary').textContent = `${(index.sourceBytes / 1048576).toFixed(2)} MiB · ${index.toc.length} sections`
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
    navigation += 1
    generation += 1
    activeRequest?.abort()
    activeRequest = null
    requestedTarget = null
    forcePage(null)
    virtualizer.cancelScroll()
  }, { buttons: !editable })
  tables = tableController(index, api, article => {
    if (article.isConnected) {
      virtualizer?.measureElement(article)
      if (article.dataset.ready && metrics.firstContentMs == null) metrics.firstContentMs = performance.now() - started
    }
    schedule()
  })
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
    changeLayout(() => shell.classList.toggle('no-wrap', !event.target.checked))
  }, { signal: readerAbort.signal })
  window.readerExperiment = { index, documentId, metrics, jump, virtualizer, cache, readingAnchor, position, diagrams, diagramMetrics, tables, source, editor, get pendingNavigation() { return forcedPage } }
}

function teardown() {
  navigation += 1
  generation += 1
  activeRequest?.abort()
  activeRequest = null
  requestedTarget = null
  readerAbort?.abort()
  if (frame != null) cancelAnimationFrame(frame)
  if (mountFrame != null) cancelAnimationFrame(mountFrame)
  frame = mountFrame = null
  source?.dispose(); source = null
  tables?.dispose(); tables = null
  diagrams?.dispose(); diagrams = null
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
  if (!response.editable || !response.documentId || comparisonPreview) return
  editable = true
  documentId = response.documentId
  document.querySelector('#view-modes').hidden = false
  document.querySelector('#edit-mode').hidden = false
  document.querySelector('.reader-footer-hint').textContent = 'Save or change editing pages to write'
  document.querySelector('#welcome small').innerHTML = 'Ctrl / ⌘ O to open · Files stay on the server<br>Use Edit to change the file, then Save or turn the page.'
  if (editor) return
  editor = editorController(token, {
    documentId: () => documentId,
    saved(next) { documentId = next.documentId; expectedRevision = next.revision },
    pause() {
      navigation += 1
      generation += 1
      activeRequest?.abort()
      requestedTarget = null
      document.querySelector('#workspace').hidden = true
      setContents(false); setSearch(false)
    },
    changed(state) {
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
  while (true) {
    const response = await api('/api/document')
    if (response.state === 'error') throw Error(response.error)
    if (response.state === 'idle') throw Error('Open a document first')
    if (response.state === 'ready') {
      if (targetRevision && response.index.revision !== targetRevision) throw Error('The reading index does not match the saved file. Reopen the file to refresh it.')
      documentId = response.documentId
      index = response.index
      for (const page of response.initial) remember(page)
      setup()
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
    document.querySelector('.reader-identity p').textContent = view === 'edit' ? 'Markdown · Paged editing' : view === 'source' ? 'Markdown · Read-only source' : 'Markdown · Read-only preview'
  } catch (error) {
    status.textContent = error.message
    document.querySelector('.reader-diagnostics').open = true
  } finally { switching = false }
}
for (const view of ['preview', 'source', 'edit']) document.getElementById(view + '-mode').addEventListener('click', () => switchView(view))

addEventListener('pagehide', () => {
  if (!comparisonPreview) writePosition(token, documentId, position())
  stopped = true
  teardown()
  editor?.dispose()
  files.dispose()
  clearInterval(heartbeat)
}, { once: true })

function setContents(open) {
  document.querySelector('#reader-outline').hidden = !open
  document.querySelector('#contents-backdrop').hidden = !open
  document.querySelector('#contents-toggle').setAttribute('aria-expanded', String(open))
  if (open) { tocVirtualizer?._willUpdate(); if (tocVirtualizer) renderToc(tocVirtualizer) }
}

function setSearch(open) {
  if (!open) searchGeneration += 1
  document.querySelector('#search').hidden = !open
  document.querySelector('#search-toggle').setAttribute('aria-expanded', String(open))
  document.querySelector('#matches').hidden = !open
  if (open) document.querySelector('#query').focus()
}

document.querySelector('#contents-toggle').addEventListener('click', async () => {
  if (!document.querySelector('#reader-outline').hidden) { setContents(false); return }
  if (switching || contentsLoading || editor?.busy) return
  contentsLoading = true
  switching = true
  document.getElementById('contents-toggle').disabled = true
  try {
    if (editor?.visible && editor.needsRefresh) await refreshReading()
    if (!index || editor?.busy) return
    if (editor?.visible && editor.revision !== index.revision) throw Error('The outline is refreshing. Reopen Contents to try again.')
    if (editor?.visible) activeHeading = index.toc.findLast(heading => heading.line <= editor.line())?.id ?? null
    document.getElementById('reader-outline').title = editor?.visible ? 'Outline of the saved file. Changes in the current editing page are mapped when jumping.' : ''
    setContents(true)
  } catch (error) {
    status.textContent = error.message
    document.querySelector('.reader-diagnostics').open = true
  } finally {
    contentsLoading = false
    switching = false
    document.getElementById('contents-toggle').disabled = Boolean(editor?.busy) || !index
  }
})
document.querySelector('#contents-close').addEventListener('click', () => setContents(false))
document.querySelector('#contents-backdrop').addEventListener('click', () => setContents(false))
document.querySelector('#search-toggle').addEventListener('click', () => setSearch(document.querySelector('#search').hidden))
document.querySelector('#search-close').addEventListener('click', () => setSearch(false))
document.addEventListener('keydown', event => { if (event.key === 'Escape') { setContents(false); setSearch(false); document.querySelector('.reader-settings').open = false } })
document.addEventListener('pointerdown', event => {
  const settings = document.querySelector('.reader-settings')
  if (settings.open && !settings.contains(event.target)) settings.open = false
})

document.querySelector('#search').addEventListener('submit', async event => {
  event.preventDefault()
  const ticket = ++searchGeneration
  try {
    const query = document.querySelector('#query').value
    const response = await api(`/api/search?query=${encodeURIComponent(query)}`)
    if (ticket !== searchGeneration) return
    const list = document.querySelector('#matches')
    list.replaceChildren()
    for (const match of response.matches) {
      const item = document.createElement('li')
      const button = document.createElement('button')
      button.textContent = `${match.line}: ${match.snippet}`
      button.addEventListener('click', () => { list.hidden = true; if (source?.visible) source.go(match.line); else jump(match.page, null, match.tableRow) })
      item.append(button); list.append(item)
    }
    status.textContent = `${response.matches.length} matching source lines (maximum ${response.limit}); case-sensitive`
    list.hidden = false
    if (!response.matches.length) {
      const empty = document.createElement('li')
      empty.textContent = 'No matching source lines'
      list.append(empty)
    }
  } catch (error) { if (ticket === searchGeneration) status.textContent = error.message }
})

window.marknoriaHost = {
  get state() {
    return {
      ready: Boolean(index || editor), readingReady: Boolean(index || editor?.needsRefresh), editable,
      view: editor?.visible ? 'edit' : source?.visible ? 'source' : 'preview',
      dirty: Boolean(editor?.dirty), busy: Boolean(editor?.busy || switching || contentsLoading),
      outlineOpen: document.getElementById('contents-toggle').getAttribute('aria-expanded') === 'true',
      hasOutline: Boolean(index?.toc.length), lineCount: editor?.data?.sourceLines ?? index?.sourceLines,
      size: editor?.data?.sourceBytes ?? index?.sourceBytes, documentId,
      status: editor?.visible || editor?.dirty ? document.getElementById('edit-status').textContent : '',
      error: document.getElementById('welcome-title').textContent === 'Could not open this document' ? status.textContent : '',
    }
  },
  async setView(view) {
    if (editable) return switchView(view)
    if (view === 'source') return source?.show()
    if (view === 'preview') document.getElementById('preview-mode').click()
  },
  save: () => editor?.save() ?? false,
  leave: () => editor?.leaving(),
  async navigate(match) {
    if (editor?.visible) {
      if (editor.needsRefresh) await refreshReading()
      return editor.go(match.line, index.revision)
    }
    if (source?.visible) return source.go(match.line)
    return jump(match.page, null, match.tableRow)
  },
}

try {
  if (!token) throw new Error('Open the full URL printed by the reader, including its #token')
  while (!index) {
    const response = await api('/api/document')
    configureEditor(response)
    if (response.state === 'idle') { files.state('idle'); clearInterval(heartbeat); break }
    if (response.state === 'error') throw new Error(response.error)
    if (response.state === 'ready') {
      documentId = response.documentId
      index = response.index
      for (const page of response.initial) remember(page)
      setup()
      const saved = readPosition(token, documentId)
      if (!new URLSearchParams(location.search).has('line') && pendingLine === 1) {
        if (saved.view === 'edit' && editable) await switchView('edit')
        else if (saved.view === 'source') await source.show(saved.line)
        else if (saved.line > 1) navigateLine(saved.line, true)
      } else if (pendingLine > 1) navigateLine(pendingLine)
      if (comparisonPreview) parent.postMessage({ type: 'marknoria:ready', documentId }, location.origin)
      if (embedded) setInterval(() => api('/api/renderer').catch(() => {}), 30_000)
      break
    }
    files.state('indexing')
    await new Promise(resolve => setTimeout(resolve, 150))
  }
} catch (error) {
  status.textContent = error.message
  files.state('error', error.message)
  document.querySelector('.reader-diagnostics').open = true
  clearInterval(heartbeat)
}
