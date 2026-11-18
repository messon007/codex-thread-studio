import { mountCommentMarks } from './comment-marks.mjs'
import { sourceCommentContext } from './source-comment-marks.mjs'
import { commentScopes } from './comment-ranges.mjs'
import { captureProjectionComment } from './comment-ranges.mjs'
import { validatePassage, resolveNotePassage, revealNotePassage, focusNotePassage } from './note-passage.mjs'
import { createTranslator, uiLabels } from './translation.mjs'
import { readerPresentation } from './presentation.mjs'
import { readerUiLabels } from './reader-ui.mjs'
import { publicReader } from './public-reader.mjs'
import { Virtualizer, observeElementRect, observeElementOffset, elementScroll, defaultRangeExtractor } from './vendor/virtual-core.mjs'
import { projectionHtml } from './projection-html.mjs'
import { diagramController, diagramMetrics } from './diagrams.mjs'
import { assetController } from './assets.mjs'
import { imageFrameResponse } from './image-frames.mjs'
import { notesController } from './notes.mjs'
import { mountReferences } from './references.mjs'
import { jqController } from './jq-query.mjs'
import { tableController } from './tables.mjs'
import { fileController } from './files.mjs'
import { sourceController } from './source.mjs'
import { editorController } from './editor.mjs'
import { readPosition, writePosition } from './position.mjs'
import { mountOutline } from './outline.mjs'
import { searchController } from './search-controller.mjs'
import { sourceTarget, visibleSourceLocation, searchHighlights } from './search-position.mjs'
import { mountGoToLine, sourcePageForLine } from './go-to-line.mjs'
import { mountWordHighlights } from './word-highlights.mjs'

const mainLifetime = new AbortController()
const translate = createTranslator()
const labels = uiLabels(translate, { signal: mainLifetime.signal })
const on = (target, type, listener, options = {}) => target.addEventListener(type, listener, { ...options, signal: mainLifetime.signal })
const embedded = new URLSearchParams(location.search).has('embedded')
const sourceFlow = new URLSearchParams(location.search).get('sourceFlow')
const token = location.hash.slice(1)
let editor = null
let editable = false
let expectedRevision = null
let switching = false
let contentsLoading = false
let readerAbort = null
let stopReader = null
let outlineView = null
let formatOutlineOwner = null
const files = fileController(token, { translate, enabled: !embedded, onOpen: () => bridge?.publish('openRequested', publicState()), beforeChange: () => !editor || editor.confirmLeave(), leaving: () => editor?.leaving() })
on(document.querySelector('#compare-files'), 'click', () => {
  if (window.readerExperiment) writePosition(token, documentId, position())
  if (editor && !editor.confirmLeave()) return
  editor?.leaving()
  clearTextSelection()
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
  if (source?.visible) return { ...source.currentLocation(), view: 'source' }
  const page = readingAnchor()
  const node = nodes.get(tables.itemForPage[page])
  return { ...visibleSourceLocation(node, viewport, { line: index.pages[page]?.line || 1, start: index.pages[page]?.start || 0 }), view: 'preview' }
}
const viewport = document.querySelector('#viewport')
const space = document.querySelector('#space')
const highlights = searchHighlights(space)
const toc = document.querySelector('#toc')
const tocSpace = document.querySelector('#toc-space')
const status = document.querySelector('#status')
function setStatus(message, values) {
  if (values == null) { labels.clear(status); status.textContent = message }
  else labels.set(status,message,values)
}
const shell = document.querySelector('.reader-shell')
const presentation = readerPresentation(shell, { translate, themeRoot: document.documentElement })
const wordHighlights = mountWordHighlights(space, { viewport, signal: mainLifetime.signal })
const goToLine = mountGoToLine(shell, {
  translate, signal: mainLifetime.signal,
  state() {
    if (!index || formatRenderer || !publicState().capabilities.source) return null
    return { identity: documentId, view: position().view, line: editor?.visible ? editor.line() : position().line,
      total: editor?.visible ? editor.data?.sourceLines : index.sourceLines,
      busy: switching || editor?.busy || source?.busy }
  },
  async navigate(line, captured) {
    if (documentId !== captured.identity || position().view !== captured.view) throw Error(translate('The document or view changed. Open Go to line again.'))
    const intent = beginNavigation()
    search.suspend()
    if (editor?.visible) {
      if (!await editor.go(line)) throw editor.error || Error(translate('Could not navigate to this source line.'))
    } else if (source?.visible) {
      if (!await source.go(line)) throw Error(translate('Could not navigate to this source line.'))
      source.code?.focus()
    } else {
      if (await previewLine({ line }, { intent }) == null) throw Error(translate('Could not navigate to this source line.'))
      viewport.focus()
    }
    bridge?.publish('state', publicState())
  }
})
function syncWordHighlights() {
  const enabled = presentation.state().highlightSelectionMatches
  const supported = Boolean(index && ['markdown','csv','tsv','text','html'].includes(index.format || 'markdown'))
  wordHighlights.update({ enabled: Boolean(enabled && supported && !formatRenderer && !source?.visible && !editor?.visible) })
  source?.code?.wordHighlights(Boolean(enabled && source.visible && !editor?.visible))
  const input = document.getElementById('highlight-selection-matches')
  input.checked = enabled
  input.disabled = !supported || Boolean(formatRenderer || source?.external || editor?.visible)
  labels.set(document.getElementById('word-highlight-scope'), input.disabled ? 'Matching-word highlights are unavailable in this view.' : 'Nearby text only. Not available in PDF, images or book readers.')
  syncReadingTypography()
}
function syncReadingTypography() {
  const state = presentation.state(), supported = Boolean(index && !formatRenderer && !index.renderer)
  const textSize = document.getElementById('reading-text-size'), codeSize = document.getElementById('reading-code-size')
  textSize.value = state.contentFontSize
  codeSize.value = state.codeFontSize ?? ''
  textSize.disabled = !supported || Boolean(editor?.visible || source?.visible)
  codeSize.disabled = !supported
  for (const id of ['column-width', 'wrap']) document.getElementById(id).disabled = !supported || Boolean(editor?.visible || source?.visible)
}
const layoutProperties = ['contentFontSize', 'codeFontSize', 'contentFontFamily', 'codeFontFamily', 'contentFontWeight', 'fontFamily', 'fontSize', 'wrap', 'toolbar']
const layoutChanged = (before, after) => layoutProperties.some(key => before[key] !== after[key])
function typographyAnchor() {
  const code = editor?.visible ? editor.code : source?.visible ? source.code : null
  const preview = index && !formatRenderer && !editor?.visible && !source?.visible ? position() : null
  const node = preview && nodes.get(tables.itemForPage[readingAnchor()])
  const target = node?.dataset.ready ? sourceTarget(node, preview) : null
  return { code, scroll: code?.scrollSnapshot(), preview, target,
    top: target?.getBoundingClientRect().top - viewport.getBoundingClientRect().top,
    owner: index, reader: virtualizer, intent: navigationIntent, navigation, pending: forcedPage != null }
}
function remeasurePreview(anchor, invalidate) {
  const reader = virtualizer
  if (!reader || anchor.owner !== index || anchor.reader !== reader || source?.visible || editor?.visible || stopped) return
  // Only mounted nodes are measured. An unchanged DOM never needs a fresh RO
  // notification, and measureElement may intentionally skip reads while scrolling.
  const mounted = [...nodes].filter(([, node]) => node.isConnected).map(([item, node]) => [item, node.offsetHeight])
  const adjust = reader.shouldAdjustScrollPositionOnItemSizeChange
  reader.shouldAdjustScrollPositionOnItemSizeChange = () => false
  try {
    if (invalidate) reader.measure()
    reader.getTotalSize() // Rebuild the cleared measurement index before resizeItem.
    for (const [item, height] of mounted) reader.resizeItem(item, height)
  } finally { reader.shouldAdjustScrollPositionOnItemSizeChange = adjust }
  commitVirtualGeometry()
  const target = anchor.target, connected = target instanceof Range ? target.startContainer.isConnected : target?.isConnected
  // A settings ACK must not restart/cancel a pending jump or pull against a
  // subsequent user navigation. Keep the same visible source block and offset.
  if (!anchor.pending && forcedPage == null && anchor.intent === navigationIntent && anchor.navigation === navigation && connected && Number.isFinite(anchor.top)) {
    const delta = target.getBoundingClientRect().top - viewport.getBoundingClientRect().top - anchor.top
    if (Math.abs(delta) > 0.5) viewport.scrollTop += delta
  }
}
async function refreshTypography(anchor, { invalidate = true } = {}) {
  syncReadingTypography()
  anchor.code?.remeasure(anchor.scroll)
  if (anchor.preview) remeasurePreview(anchor, invalidate)
  highlights.refresh(); wordHighlights.refresh(); commentMarks?.refresh()
  if (!anchor.code) search.repaint()
  schedule()
}
// Fonts can finish loading after a settings acknowledgement. Re-read only the
// current mounted set; do not revive an earlier settings/navigation anchor.
on(document.fonts, 'loadingdone', () => { if (!stopped) void refreshTypography(typographyAnchor(), { invalidate: false }) })
for (const [id, property] of [['reading-text-size', 'contentFontSize'], ['reading-code-size', 'codeFontSize']]) {
  on(document.getElementById(id), 'change', async event => {
    const input = event.target, before = presentation.state()
    const value = input.value === '' && property === 'codeFontSize' ? null : Number(input.value)
    if (value !== null && (!input.value || !Number.isFinite(value) || value < 11 || value > 32)) { input.value = before[property] ?? ''; return }
    if (value === before[property]) return
    const anchor = typographyAnchor()
    presentation.update({ [property]: value })
    if (!embedded) try {
      const state = presentation.state()
      localStorage.setItem('marknoria-reading-font-sizes', JSON.stringify({ contentFontSize: state.contentFontSize, codeFontSize: state.codeFontSize }))
    } catch {}
    try { await refreshTypography(anchor); bridge?.publish('state', publicState()) }
    catch (error) { if (error.name !== 'AbortError') setStatus(error.message, null) }
  })
}
if (!embedded) try {
  const stored = JSON.parse(localStorage.getItem('marknoria-reading-font-sizes') || '{}')
  const patch = {}
  for (const key of ['contentFontSize', 'codeFontSize']) if (stored?.[key] === null && key === 'codeFontSize' || typeof stored?.[key] === 'number' && stored[key] >= 11 && stored[key] <= 32) patch[key] = stored[key]
  presentation.update(patch)
} catch {}
on(document.getElementById('highlight-selection-matches'), 'change', event => {
  presentation.update({ highlightSelectionMatches: event.target.checked })
  try { localStorage.setItem('marknoria-highlight-selection-matches', String(event.target.checked)) } catch {}
  syncWordHighlights(); bridge?.publish('state', publicState())
})
try { presentation.update({ highlightSelectionMatches: localStorage.getItem('marknoria-highlight-selection-matches') === 'true' }) } catch {}
// Presentation-only startup policy: no authority or storage grants.
{
  const startup = new URLSearchParams(location.search)
  presentation.update({
    ...(startup.get('notes') === 'host' ? { notes: 'host' } : {}),
    ...(startup.get('selectionMenu') === 'host' ? { selectionMenu: 'host' } : {})
  })
}
readerUiLabels(shell, translate, { signal: mainLifetime.signal })
on(window, 'marknoria:theme', event => { if (['light','dark'].includes(event.detail?.theme)) { presentation.update({ theme: event.detail.theme }); bridge?.publish('state', publicState()) } })
let activeHeading = null
const cache = new Map()
const nodes = new Map()
let diagrams = null
let assets = null
let documentReferences = null
let notes = null
let commentMarks = null
let commentMarksOwner = null
function commentContext() {
  if (editor?.visible) return !editor.dirty && !editor.needsRefresh ? sourceCommentContext(editor.code, editor.data?.window) : { scopes: [] }
  if (source?.visible) return sourceCommentContext(source.code, source.page)
  if (formatRenderer) return { scopes: formatRenderer.commentScopes?.() || [], resolve: records => formatRenderer?.resolveCommentMarks?.(records) || [] }
  return { scopes: commentScopes(shell, { viewport }), viewport }
}
function syncNotesOwner() {
  const owned = serviceCapabilities.notes && presentation.state().notes === 'reader'
  if (!owned) { notes?.dispose(); notes = null }
  else if (index && !notes) notes = notesController(index,api,()=>!editor?.dirty,signal => publicSelection(signal, { textOnly: true }),{documentId,translate,selectionMenu:presentation.state().selectionMenu,navigate:navigateNote,onMutation:()=>commentMarks?.refresh(true)})
  const identity = index && `${documentId}:${index.revision}:${owned}`
  if (identity !== commentMarksOwner) {
    commentMarks?.dispose(); commentMarks = null; commentMarksOwner = identity
    if (index) commentMarks = mountCommentMarks({ owner:{documentId,revision:index.revision}, root:shell, viewport, context:commentContext, translate,
      load: owned ? (request,{signal}) => api('/api/notes',signal,{method:'POST',body:JSON.stringify(request)}) : null,
      open: id => owned ? (id ? notes?.openComment(id) : notes?.open(0,'comment')) : bridge?.publish('commentRequested',{documentId,revision:index.revision,id}),
      onStatus: message => { if (message) shell.dataset.commentMarksError=message; else delete shell.dataset.commentMarksError }
    })
  }
}
let jq = null
let cacheBytes = 0
let index
let virtualizer
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
let navigationIntent = 0
let passageRequest = null
let locationRequest = null
let passageFocus = null
function beginNavigation() {
  passageRequest?.abort()
  locationRequest?.abort()
  passageFocus = null
  return ++navigationIntent
}
let documentError = null
let loadError = null
let searchJump = null
let searchPreferenceDocument = null
const search = searchController({
  owner: () => ({ documentId, revision: index?.revision, defaultMatchCase: index?.renderer !== 'pdfjs', renderer: Boolean(formatRenderer), source: Boolean(index && ['markdown','csv','tsv','text','html'].includes(index.format)), editing: Boolean(editor?.visible) }),
  load: (query, offset, owner, signal, { matchCase }) => formatRenderer
    ? formatRenderer.search(query, { signal, matchCase })
    : api(`/api/search?query=${encodeURIComponent(query)}&offset=${offset}&revision=${encodeURIComponent(owner.revision)}&matchCase=${matchCase}`, signal),
  describe(match) {
    if (formatRenderer) return { ...match, locationPrecision: match.locationPrecision || 'exact' }
    const table = index?.pages[match.page]?.table
    return { ...match, table, location: table != null && match.tableRow != null
      ? { kind: 'cell', table, row: match.tableRow, column: 0 } : { kind: 'line', line: match.line },
      locationPrecision: table != null ? 'row' : 'line', ...(['markdown','csv','tsv','text','html'].includes(index?.format) ? { sourceLocation: { kind: 'source', startByte: match.start, endByte: match.end } } : {}) }
  },
  async activate(match, { query, matchCase, signal }) {
    signal.throwIfAborted()
    if (formatRenderer) { await formatRenderer.activateSearch(match, { query, matchCase, signal }); return }
    if (source?.visible) { highlights.clear(); await source.goRange(match.start, match.end, { signal, search: true }); return }
    source?.clearSearch()
    highlights.set(query, match, { matchCase })
    const before = navigation
    const pending = jump(match.page, null, match.tableRow, null, match, query, { matchCase })
    const owned = navigation !== before ? navigation : null
    searchJump = owned
    const abort = () => { if (searchJump === owned) cancelSearchNavigation() }
    signal.addEventListener('abort', abort, { once: true })
    try {
      const completed = await pending; signal.throwIfAborted()
      if (owned === null || completed !== owned || navigation !== owned)
        throw new DOMException('Search navigation superseded', 'AbortError')
      // Search scans the immutable source, including omitted preview tails.
      // Resolve these matches in Source rather than claiming that the visible
      // cell prefix contains the selected occurrence.
      const article = nodes.get(tables.itemForPage[match.page])
      const sourceRow = article?.querySelector(`tr[data-source-line="${match.line}"]`)
      if (sourceRow?.matches('[data-row-preview="true"]') || sourceRow?.querySelector('[data-cell-preview="true"]')) {
        highlights.clear()
        await source.goRange(match.start, match.end, { signal, search: true })
      }
    }
    finally { signal.removeEventListener('abort', abort); if (searchJump === owned) searchJump = null }
  },
  paint(query, active, { matchCase }) {
    if (formatRenderer) {
      if (!query || !active) formatRenderer.clearSearch?.({ keepResults: true })
      else formatRenderer.repaintSearch?.(active, { query, matchCase })
      return
    }
    if (source?.visible) { highlights.clear(); if (active) source.highlightRange(active.start, active.end); else source.clearSearch() }
    else { source?.clearSearch(); highlights.set(query, active, { matchCase }) }
  },
  cancel: cancelSearchNavigation,
  clear() { highlights.clear(); source?.clearSearch(); formatRenderer?.clearSearch?.() },
  changed() {
    if (mainLifetime.signal.aborted) return
    const state = search.state()
    if (state.searchId) {
      document.getElementById('query').value = state.query
      document.getElementById('search-match-case').checked = state.matchCase
    }
    updateSearchCounter(); renderSearchResults(); bridge?.publish('state', publicState())
  },
})
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
  const { imageRevision, ...requestOptions } = options
  const computing = options.method === 'POST' && ['/api/table-query', '/api/jq-query', '/api/table-chart'].includes(path)
  if (computing) { queryBusy++; bridge?.publish('state', publicState()) }
  try {
    for (let attempt = 0; attempt < 4; attempt += 1) {
      const response = await fetch(new URL(`.${path}`, import.meta.url), { ...requestOptions, headers: { ...(options.body ? { 'Content-Type': 'application/json' } : {}), ...options.headers, 'x-reader-token': token, ...(documentId ? { 'x-reader-document': documentId } : {}) }, signal })
      if (response.status === 429 && attempt < 3) {
        await new Promise(resolve => setTimeout(resolve, 50 * 2 ** attempt))
        continue
      }
      if (!response.ok) throw Object.assign(new Error(`Reader request failed (${response.status}): ${(await response.text()).slice(0, 800)}`),{status:response.status,...(response.status === 429 ? {code:'BUSY'} : {})})
      if (response.status === 202 && imageRevision !== undefined) throw Object.assign(Error('Image snapshot is preparing'),{code:'BUSY'})
      if (imageRevision !== undefined) return imageFrameResponse(response, { revision: imageRevision, signal })
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
  setStatus(loadError || `${(index.sourceBytes / 1048576).toFixed(2)} MiB · ${index.pages.length} fragments · ${index.toc.length} headings · index ${index.indexMs.toFixed(0)} ms · ${nodes.size} mounted fragments · ${(cacheBytes / 1048576).toFixed(1)} MiB HTML cache`, null)
  if (loadError) document.querySelector('.reader-diagnostics').open = true
}

function schedule() {
  if (frame == null && !stopped) frame = requestAnimationFrame(() => { frame = null; reconcile() })
}

// A resize scroll adjustment and the corresponding transforms/sizer must commit
// together. This only updates mounted geometry; mounting/providers stay scheduled.
function commitVirtualGeometry(items = virtualizer?.getVirtualItems()) {
  if (!items || stopped || source?.visible || editor?.visible) return
  space.style.height = `${virtualizer.getTotalSize()}px`
  const measurements = virtualizer.getMeasurements()
  for (const [index, node] of nodes) {
    const item = measurements[index]
    if (item && node.isConnected) node.style.transform = `translateY(${item.start}px)`
  }
}

// Provider readiness can occur during user scrolling, when TanStack defers
// measureElement to RO. Commit the known mounted height before publishing ready.
function measurePage(node) {
  if (!virtualizer || !node?.isConnected) return
  virtualizer.measureElement(node)
  virtualizer.getTotalSize()
  virtualizer.resizeItem(Number(node.dataset.index), node.offsetHeight)
  commitVirtualGeometry()
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
  const resized = space.style.width !== `${viewport.clientWidth}px` ? typographyAnchor() : null
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
      const loadingLabel = document.createElement('span')
      labels.set(loadingLabel,'Loading nearby content…'); node.append(loadingLabel)
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
      catch (error) { loadError = error.message; labels.clear(mount.node); mount.node.textContent = loadError; mount.node.dataset.ready = 'true'; mount.node.classList.remove('placeholder'); report(); return }
      mount.node.dataset.ready = 'true'
      mount.node.classList.remove('placeholder')
      diagrams.mount(mount.node)
      assets?.mount(mount.node)
      measurePage(mount.node)
      if (metrics.firstContentMs == null && !['image','pdf'].includes(index.format)) metrics.firstContentMs = performance.now() - started
      report()
      schedule()
    })
  }
  if (missing != null && !loadError) requestWindow(missing)
  virtualizer._willUpdate()
  if (resized) remeasurePreview(resized, false)
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

async function previewLine(location, options = {}) {
  if (typeof location === 'number') location = { line: location }
  const { line } = location
  if (!index?.pages.length) return
  const snapshot = index, intent = options.intent ?? ++navigationIntent
  options = { ...options, intent }
  const page = Number.isSafeInteger(location.start) ? Math.min(index.pages.length - 1, Math.max(0, index.pages.findIndex(page => page.end > location.start))) : sourcePageForLine(index.pages, line)
  const descriptor = index.pages[page]
  let row = null
  if (descriptor.table != null) {
    const response = await api(`/api/window?target=${page}`, options.signal || mainLifetime.signal)
    options.signal?.throwIfAborted()
    if (index !== snapshot || intent !== navigationIntent) return
    const content = response.pages.find(item => item.page === page || item.id === page)
    if (content) {
      const fragment = projectionHtml(content.html)
      const rows = [...fragment.querySelectorAll('tbody tr[data-source-line]')]
      const offset = rows.findLastIndex(node => Number(node.dataset.sourceLine) <= line)
      if (offset >= 0) row = descriptor.rowStart + offset
    }
  }
  return jump(page, null, row, null, location, '', {}, options)
}

async function jump(sourcePage, heading = null, row = null, anchor = null, location = null, query = '', searchOptions = {}, { intent = beginNavigation(), signal, keepMounted = false } = {}) {
  signal?.throwIfAborted()
  if (intent !== navigationIntent || stopped || !index || !tables || !virtualizer || !index.pages[sourcePage]) return
  if (editor?.visible) {
    const target = heading == null ? index.pages[sourcePage]?.line : index.toc.find(item => item.id === heading)?.line
    if (await editor.go(target, index.revision)) {
      activeHeading = heading
      outlineView?.active(activeHeading)
    }
    return
  }
  if (source?.visible) {
    activeHeading = heading ?? index.toc.findLast(item => item.page <= sourcePage)?.id ?? null
    outlineView?.active(activeHeading)
    const line = heading == null ? index.pages[sourcePage].line : index.toc.find(item => item.id === heading)?.line
    await source.go(line, { signal })
    return
  }
  const page = tables.target(sourcePage, row)
  activeHeading = heading ?? index.toc.findLast(item => item.page <= sourcePage)?.id ?? null
  outlineView?.active(activeHeading)
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
  const cancel = () => { if (ticket === navigation) { navigation++; forcePage(null); virtualizer.cancelScroll(); schedule() } }
  signal?.addEventListener('abort', cancel, { once: true })
  let completed = false
  try {
    let stable = 0
    let previous = null
    for (let attempt = 0; attempt < 150 && ticket === navigation; attempt += 1) {
      await new Promise(resolve => setTimeout(resolve, 20))
      signal?.throwIfAborted()
      if (stopped || intent !== navigationIntent || ticket !== navigation || !index || !tables || !virtualizer) return
      const article = nodes.get(page)
      const tableRow = row ?? index.pages[sourcePage]?.rowStart
      const target = location ? sourceTarget(article, location, query, searchOptions) : anchor != null ? article?.querySelector(`[data-document-anchor="${CSS.escape(anchor)}"]`) : heading == null
        ? (tableRow > Number(article?.dataset.rowStart || 0) ? article?.querySelectorAll('tbody tr')[tableRow - Number(article.dataset.rowStart)] : article)
        : document.getElementById(`heading-${heading}`)
      const targetNode = target instanceof Range ? target.startContainer.parentElement : target
      const diagramsReady = article && targetNode && [...article.querySelectorAll('figure[data-diagram-state]')].every(figure => !(figure.compareDocumentPosition(targetNode) & Node.DOCUMENT_POSITION_FOLLOWING) || ['rendered', 'error'].includes(figure.dataset.diagramState))
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
    completed = ticket === navigation && intent === navigationIntent
    if (completed) return ticket
  } finally {
    signal?.removeEventListener('abort', cancel)
    if (ticket === navigation && (!completed || !keepMounted)) { forcePage(null); schedule() }
  }
}

function createOutline(navigate) {
  return mountOutline({ viewport: toc, space: tocSpace,
    filter: document.querySelector('#toc-filter'), feedback: document.querySelector('#toc-feedback'),
    navigate, translate, onError: error => { setStatus(error.message, null) } })
}

async function navigateFragment(fragment) {
  const anchor = index.anchors?.find(anchor => anchor.name === fragment)
  if (anchor) return jump(anchor.page,null,null,fragment)
  const counts = new Map()
  const heading = index.toc.find(heading => {
    const slug = heading.label.toLowerCase().replace(/[^\p{L}\p{M}\p{N}_\s-]/gu,'').replace(/\s/gu,'-')
    const count = counts.get(slug) || 0; counts.set(slug,count+1)
    return fragment === `heading-${heading.id}` || fragment === slug+(count?'-'+count:'')
  })
  if (!heading) throw Error('Document fragment target is unavailable')
  await jump(heading.page,heading.id)
}

function setup() {
  if (searchPreferenceDocument !== documentId) {
    document.getElementById('search-match-case').checked = search.capabilities().defaultMatchCase
    searchPreferenceDocument = documentId
  }
  if(window.readerExperiment){started=performance.now();Object.assign(metrics,{firstContentMs:null,firstAssetMs:null,maxHeartbeatGapMs:0});heartbeatAt=started}
  readerAbort = new AbortController()
  if (['pdfjs','epubjs'].includes(index.renderer)) { setupFormat(); return }
  diagrams = diagramController(viewport, article => { measurePage(article); schedule() }, { documentId, revision: index.revision, token })
  assets = assetController(viewport,index,documentId,article=>{if(metrics.firstAssetMs==null)metrics.firstAssetMs=performance.now()-started;if(metrics.firstContentMs==null&&['image','pdf'].includes(index.format))metrics.firstContentMs=performance.now()-started;if(article)measurePage(article);schedule()},{translate})
  syncNotesOwner()
  documentReferences = index.format === 'markdown' ? mountReferences(space, {
    owner: { documentId, revision: index.revision }, viewport, translate,
    provider: {
      info: ({id,signal}) => api(`/api/reference?id=${id}&revision=${encodeURIComponent(index.revision)}`,signal),
      imageInfo: ({id,signal}) => api(`/api/reference-image?id=${id}&revision=${encodeURIComponent(index.revision)}`,signal),
      imageFrame: ({id,imageRevision,frame,signal}) => api(`/api/reference-image?id=${id}&revision=${encodeURIComponent(index.revision)}&imageRevision=${encodeURIComponent(imageRevision)}&frame=${frame}`,signal,{imageRevision:index.revision}),
    },
    onChange: article => { if (article) measurePage(article); bridge?.publish('state',publicState()) },
    onLink: async value => {
      const target = value.target
      if (target?.startsWith('#')) {
        await navigateFragment(decodeURIComponent(target.slice(1)))
      } else if (embedded) {
        if (!bridge?.connected) throw Error('The host has not connected its link handler')
        bridge.publish('linkRequested',value)
      } else if (value.openPath) await files.open(value.openPath)
      else if (/^(https?:|mailto:)/i.test(target || '')) window.open(target,'_blank','noopener,noreferrer')
      else throw Error('Reference is outside the authorized folder or uses an unsupported scheme')
    },
  }) : null
  jq = serviceCapabilities.queries ? jqController(index,api,{translate}) : null
  files.state('ready')
  document.querySelector('#document-name').textContent = index.name || 'Document'
  document.title = `${index.name || 'Document'} — MarkNoria`
  labels.set(document.querySelector('.reader-identity p'),'{format} · Preview',{format:(index.format||'markdown').toUpperCase()})
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
    schedule()
    return previewLine(line)
  }, () => position(), ({ intent } = {}) => {
    // A byte-range navigation owns this mode transition. Preserve that exact
    // intent, but never adopt an obsolete caller's intent over a newer command.
    if (intent != null && intent !== navigationIntent) throw new DOMException('Navigation superseded', 'AbortError')
    clearTextSelection()
    tables?.closeOverlays()
    if (intent == null) beginNavigation()
    navigation += 1
    generation += 1
    activeRequest?.abort()
    activeRequest = null
    requestedTarget = null
    forcePage(null)
    virtualizer.cancelScroll()
  }, {
    buttons: !editable,
    flow: comparisonPreview ? 'paged' : sourceFlow,
    contextual: !comparisonPreview,
    translate,
    onVisibilityChange: ({ preserveSearch = false } = {}) => {
      if (!preserveSearch) search.suspend()
      clearTextSelection(); highlights.refresh(); wordHighlights.clear()
    },
    onReady: () => { syncWordHighlights(); commentMarks?.refresh(); search.repaint(); bridge?.publish('state', publicState()) },
  })
  tables = tableController(index, api, article => {
    if (article.isConnected) {
      measurePage(article)
      if (article.dataset.ready && metrics.firstContentMs == null) metrics.firstContentMs = performance.now() - started
    }
    schedule()
  }, { translate, queries: serviceCapabilities.queries, onSelection: () => { notes?.selectionChanged(); document.dispatchEvent(new Event('selectionchange')) } })
  virtualizer = new Virtualizer({
    count: tables.items.length,
    getScrollElement: () => viewport,
    estimateSize: item => tables.items[item].table != null ? 500 : Math.max(60, index.pages[tables.items[item].page].bytes / 3),
    // The virtualizer only notifies when its visible range changes. A width
    // resize can keep that range, but our prose/table width still needs layout.
    observeElementRect: (instance, changed) => observeElementRect(instance, rect => { changed(rect); schedule() }),
    observeElementOffset,
    scrollToFn: elementScroll,
    overscan: 1,
    onChange: (_instance, sync) => { if (sync) commitVirtualGeometry(); schedule() },
  })
  virtualizer.shouldAdjustScrollPositionOnItemSizeChange = item => item.end <= viewport.scrollTop
  stopReader = virtualizer._didMount()
  virtualizer._willUpdate()
  outlineView = createOutline(id => {
    const heading = index.toc.find(item => item.id === id)
    return jump(heading.page, heading.id)
  })
  void outlineView.setItems(index.toc).then(() => outlineView?.active(activeHeading)).catch(error => { if (!readerAbort.signal.aborted) setStatus(error.message, null) })
  schedule()
  const cancelNavigation = () => {
    beginNavigation()
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
    const anchor = typographyAnchor()
    if (update() === false) return
    void refreshTypography(anchor)
  }
  document.querySelector('#column-width').addEventListener('change', event => {
    const width = Math.min(600, Math.max(80, Number(event.target.value) || 180))
    event.target.value = width
    changeLayout(() => {
      if (Number.parseFloat(getComputedStyle(shell).getPropertyValue('--column-width')) === width) return false
      shell.style.setProperty('--column-width', `${width}px`)
    })
  }, { signal: readerAbort.signal })
  document.querySelector('#wrap').addEventListener('change', event => {
    changeLayout(() => {
      if (presentation.state().wrap === event.target.checked) return false
      presentation.update({ wrap: event.target.checked })
    })
  }, { signal: readerAbort.signal })
  window.readerExperiment = { index, documentId, metrics, jump, virtualizer, cache, readingAnchor, position, diagrams, diagramMetrics, tables, source, editor, wordHighlights, get pendingNavigation() { return forcedPage } }
  syncWordHighlights()
}

function setupFormat() {
  syncWordHighlights()
  const owner = { documentId, revision: index.revision, resourceId, signal: readerAbort.signal }
  const pdf = index.renderer === 'pdfjs', label = pdf ? 'PDF' : 'EPUB', saved = readPosition(token, documentId)
  formatLoading = true; space.style.height = pdf ? 'auto' : '100%'; space.style.width = '100%'; files.state('ready')
  document.querySelector('#document-name').textContent = index.name
  document.title = `${index.name} — MarkNoria`
  labels.set(document.querySelector('.reader-identity p'),'{format} · Preview',{format:label})
  document.querySelector('#source-mode').hidden = true; document.querySelector('#edit-mode').hidden = true
  document.querySelector('#view-modes').hidden = false; document.querySelector('#compare-files').disabled = true
  document.querySelector('#document-summary').textContent = `${(index.originalBytes / 1048576).toFixed(2)} MiB · Loading ${label}…`
  syncNotesOwner()
  formatReady = (pdf ? import('./pdf-reader.mjs') : import('./epub-reader.mjs')).then(async module => {
    owner.signal.throwIfAborted()
    const mount = pdf ? module.mountPdfReader : module.mountEpubReader
    formatRenderer = mount(space, { ...owner, viewport, api, translate,
      onShortcut: key => { if (key === 'f') setSearch(true); else if (key === 'o') files.choose() },
      initialState: saved.epub || { theme: document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light' },
      url: new URL(`./api/original?revision=${owner.revision}`, import.meta.url).href,
      headers: { 'x-reader-token': token, 'x-reader-document': owner.documentId },
      onSelection: value => {
        if (owner.signal.aborted) return
        bridge?.publish('selection', value?.quote?.trim() ? value : null)
        if (value?.quote?.trim()) notes?.selectionChanged()
        else notes?.clearSelection()
      },
      onState: value => {
        commentMarks?.refresh()
        if (owner.signal.aborted) return
        if (metrics.firstContentMs == null && value.firstUsefulContentMs != null) metrics.firstContentMs = performance.now() - started
        metrics.mountedPages = value.mountedPages ?? space.querySelectorAll('iframe').length
        document.querySelector('#document-summary').textContent = `${(index.originalBytes / 1048576).toFixed(2)} MiB · ${pdf ? `${value.pages} pages` : `${Math.round((value.progress || 0) * 100)}% · ${value.chapterLabel || 'EPUB'}`}`
        if (value.error) setStatus(value.error,null)
        else if (pdf) setStatus(value.continuous ? 'Page {page} / {pages} · Continuous' : 'Page {page} / {pages} · Paged', {page:value.page,pages:value.pages})
        else setStatus(`${value.chapterLabel || 'EPUB'} · ${value.flow || 'paginated'} · ${value.progressPrecision || 'chapter'} progress`,null)
        bridge?.publish('state', publicState())
      },
    })
    await formatRenderer.ready; owner.signal.throwIfAborted()
    if (pdf && saved.pdf) { await formatRenderer.settings({ zoom: saved.pdf.zoom, continuous: saved.pdf.continuous }); if (saved.pdf.location) await formatRenderer.navigate(saved.pdf.location) }
    formatLoading = false; bridge?.publish('state', publicState())
    window.readerExperiment = { index, documentId, metrics, formatRenderer, position }
  }).catch(error => { formatLoading = false; if (!owner.signal.aborted) { documentError = error.message; setStatus(error.message, null); files.state('error', error.message) } throw error })
  void formatReady.catch(() => {})
}

async function renderPdfOutline() {
  const owner = formatRenderer, signal = readerAbort.signal
  if (formatOutlineOwner === owner) return
  const items = []
  let offset = 0, total
  do {
    const page = await owner.outline({ offset, resolve: false, signal })
    if (signal.aborted || owner !== formatRenderer) return
    total = page.total
    items.push(...page.items); offset += page.items.length
    if (offset < total && !page.items.length) throw Error('Incomplete outline metadata')
  } while (offset < total)
  formatOutlineTotal = total
  outlineView?.dispose()
  outlineView = createOutline(async id => {
    await owner.navigateOutline(id, { signal })
    if (!signal.aborted && owner === formatRenderer) outlineView?.active(id)
  })
  await outlineView.setItems(items)
  if (signal.aborted || owner !== formatRenderer) return
  formatOutlineOwner = owner
  document.querySelector('#section-count').textContent = total
}

function clearTextSelection() {
  const code = editor?.visible ? editor.code : source?.visible ? source.code : null
  if (code && !code.view.state.selection.main.empty) code.view.dispatch({ selection: { anchor: code.view.state.selection.main.head } })
  notes?.clearSelection()
  getSelection()?.removeAllRanges()
  bridge?.publish('selection', null)
  document.dispatchEvent(new Event('selectionchange'))
}

function teardown() {
  beginNavigation()
  goToLine.close(); wordHighlights.clear(); wordHighlights.update({ enabled: false })
  clearTextSelection()
  clearSearchResults()
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
  documentReferences?.dispose(); documentReferences = null
  commentMarks?.dispose(); commentMarks = null; commentMarksOwner = null
  notes?.dispose(); notes = null
  jq?.dispose(); jq = null
  stopReader?.(); outlineView?.dispose()
  stopReader = outlineView = formatOutlineOwner = null
  virtualizer = null
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
  labels.set(document.querySelector('.reader-footer-hint'),'Save or change editing pages to write')
  document.querySelector('#welcome small').innerHTML = window.marknoriaDesktop
    ? 'Ctrl / ⌘ O to open · Choose files from any local folder<br>Use Edit to change the file, then Save or turn the page.'
    : 'Ctrl / ⌘ O to open · Files stay on the server<br>Use Edit to change the file, then Save or turn the page.'
  if (editor) return
  editor = editorController(token, {
    translate, nativeNotifications: !embedded,
    documentId: () => documentId,
    saved(next) { documentId = next.documentId; expectedRevision = next.revision; bridge?.publish('saved', { documentId, revision: next.revision, previewRevision: index?.revision, needsRefresh: true }) },
    pause() {
      beginNavigation()
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
      for (const id of ['preview-mode', 'source-mode', 'edit-mode', 'compare-files']) document.getElementById(id).disabled = state.busy || switching
      for (const button of document.querySelectorAll('[data-open-server]')) button.disabled = state.busy
      document.getElementById('contents-toggle').disabled = state.busy || contentsLoading || !index
      document.getElementById('search-toggle').disabled = state.visible || !index
      document.querySelector('.reader-settings').hidden = false
      syncWordHighlights()
      notice.hidden = state.visible || !state.dirty
    },
  })
}

const notice = document.createElement('div')
notice.id = 'edit-notice'
notice.hidden = true
labels.set(notice,'This view shows the saved file. Edit * keeps your unsaved page in memory; Save to refresh the reading views.')
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
    setStatus('Refreshing the saved reading view…', {})
    await new Promise(resolve => setTimeout(resolve, 100))
  }
}

async function switchView(view, { restorePreview = true } = {}) {
  if (!editable || switching || editor?.busy) return
  const current = editor.visible ? 'edit' : source?.visible ? 'source' : 'preview'
  if (view === current && index && !editor.needsRefresh) return
  switching = true
  for (const id of ['preview-mode', 'source-mode', 'edit-mode', 'compare-files']) document.getElementById(id).disabled = true
  search.suspend()
  clearTextSelection()
  const location = position()
  const line = location.line
  setContents(false)
  try {
    if (view === 'edit') {
      source?.hide()
      await editor.show(line)
    } else {
      if (!index || editor.needsRefresh) await refreshReading()
      editor.hide()
      document.getElementById('workspace').hidden = false
      if (view === 'source') await source.show(location)
      else {
        const fromSource = source.visible
        await source.hide({ restore: restorePreview })
        schedule()
        if (restorePreview && !fromSource && index.pages.length) {
          await previewLine(location)
        }
      }
    }
    for (const mode of ['preview', 'source', 'edit']) document.getElementById(mode + '-mode').setAttribute('aria-pressed', String(mode === view))
    const format = (index.format||'markdown').toUpperCase()
    labels.set(document.querySelector('.reader-identity p'),view === 'edit' ? '{format} · Paged editing' : view === 'source' ? '{format} · Read-only source' : '{format} · Read-only preview',{format})
    if (view === 'edit') search.clear(); else search.repaint()
  } catch (error) {
    setStatus(error.message, null)
    document.querySelector('.reader-diagnostics').open = true
  } finally {
    switching = false
    for (const id of ['preview-mode', 'source-mode', 'edit-mode', 'compare-files']) document.getElementById(id).disabled = Boolean(editor?.busy)
    syncWordHighlights()
    bridge?.publish('state', publicState())
  }
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
  if (open) { if (!formatRenderer) outlineView?.active(activeHeading); outlineView?.show() }
  bridge?.publish('state', publicState())
}

function setSearch(open) {
  if (!open) {
    const state = search.state()
    if (state.loading && state.current == null && !state.available) search.clear()
    else search.suspend()
  }
  else search.repaint()
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
    setStatus(error.message, null)
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
on(document.querySelector('#contents-close'), 'click', () => { setContents(false); document.querySelector('#contents-toggle').focus() })
on(document.querySelector('#contents-backdrop'), 'click', () => { setContents(false); document.querySelector('#contents-toggle').focus() })
on(document.querySelector('#search-toggle'), 'click', () => setSearch(document.querySelector('#search').hidden))
on(document.querySelector('#search-close'), 'click', () => { setSearch(false); document.querySelector('#search-toggle').focus() })
on(document, 'keydown', event => {
  if (event.defaultPrevented || document.querySelector('dialog[open]')) return
  if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'f' && index && !document.querySelector('dialog[open]')) {
    event.preventDefault(); if (editor?.visible) editor.search(); else if (source?.visible) source.search(); else setSearch(true)
  } else if (event.key === 'Escape') {
    const outlineOpen = !document.querySelector('#reader-outline').hidden, searchOpen = !document.querySelector('#search').hidden
    setContents(false); setSearch(false); document.querySelector('.reader-settings').open = false
    if (outlineOpen) document.querySelector('#contents-toggle').focus(); else if (searchOpen) document.querySelector('#search-toggle').focus()
  }
})
on(document, 'pointerdown', event => {
  const settings = document.querySelector('.reader-settings')
  if (settings.open && !settings.contains(event.target)) settings.open = false
})

function updateSearchCounter() {
  if (mainLifetime.signal.aborted) return
  const state = search.state(), counter = document.getElementById('search-count')
  counter.hidden = !state.query
  const values = { current: state.current == null ? 0 : state.current + 1, total: state.total }
  labels.set(counter, state.loading && state.current == null ? '…' : state.error ? '—' : state.totalExact ? '{current}/{total}' : '{current}/{total}+', values)
  const description = state.loading && state.current == null ? 'Searching…' : state.error ? 'Search failed'
    : state.totalExact ? '{current} of {total} matches' : '{current} of at least {total} matches; result limit reached'
  for (const attribute of ['aria-label', 'title']) labels.set(counter, description, values, attribute)
  document.getElementById('search').setAttribute('aria-busy', String(state.loading))
  for (const id of ['search-previous', 'search-next']) document.getElementById(id).disabled = state.loading || !state.available
}
function cancelSearchNavigation() {
  if (searchJump != null && searchJump === navigation) {
    navigation += 1
    forcePage(null); virtualizer?.cancelScroll(); schedule()
  }
  searchJump = null
}
function clearSearchResults() { search.clear() }
let renderedSearchWindow = null
function renderSearchResults() {
  if (mainLifetime.signal.aborted) return
  const response = search.response(), list = document.getElementById('matches')
  if (renderedSearchWindow !== response.matches) {
    renderedSearchWindow = response.matches; list.replaceChildren()
    for (const match of response.matches) {
      const item = document.createElement('li'), button = document.createElement('button')
      button.type = 'button'
      button.textContent = formatRenderer ? `${match.label || `Page ${match.page}`}: ${match.excerpt}` : `${match.line}: ${match.snippet}`
      button.dataset.matchIndex = match.index
      button.addEventListener('click', () => activateSearchMatch(match.index))
      item.append(button); list.append(item)
    }
    if (response.query && !response.loading && !response.total) { const empty = document.createElement('li'); labels.set(empty, 'No matches'); list.append(empty) }
  }
  for (const button of list.querySelectorAll('button')) {
    button.disabled = response.loading
    button.setAttribute('aria-current', String(Number(button.dataset.matchIndex) === response.current))
  }
}
async function activateSearchMatch(index) {
  const state = search.state()
  if (!state.available || state.loading) return
  document.getElementById('matches').hidden = true
  try { await search.activate({ searchId: state.searchId, index: (index + state.available) % state.available }) }
  catch (error) { if (error.name !== 'AbortError') setStatus(error.message, null) }
}
on(document.getElementById('search-previous'), 'click', () => { const state = search.state(); activateSearchMatch(state.current == null ? state.available - 1 : state.current - 1) })
on(document.getElementById('search-next'), 'click', () => activateSearchMatch((search.state().current ?? -1) + 1))
on(document.getElementById('query'), 'input', clearSearchResults)
on(document.getElementById('query'), 'keydown', event => {
  if (event.isComposing || event.keyCode === 229) return
  const state = search.state()
  if (event.key === 'Enter' && state.query === event.target.value && state.matchCase === document.getElementById('search-match-case').checked && state.available) {
    event.preventDefault(); if (!state.loading) activateSearchMatch((state.current ?? -1) + (event.shiftKey ? -1 : 1))
  }
})
async function runSearch() {
  const query = document.querySelector('#query').value
  try {
    const response = await search.search({ query, offset: 0, matchCase: document.getElementById('search-match-case').checked })
    if (!response.searchId) return
    setStatus(response.totalExact ? (response.matchCase ? '{count} matches; case-sensitive' : '{count} matches; case-insensitive') : '{count} matches · Result limit reached', { count: response.total })
    if (response.available) await search.activate({ searchId: response.searchId, index: 0 })
    if (search.state().searchId === response.searchId) document.getElementById('matches').hidden = false
  } catch (error) {
    if (!mainLifetime.signal.aborted && error.name !== 'AbortError') {
      setStatus(error.message, null)
      // Invalid input never enters the bounded shared session. Present its
      // local form error without retaining that query in controller state.
      if (!search.state().query && query) {
        const counter = document.getElementById('search-count')
        counter.hidden = false; labels.set(counter, '—')
        for (const attribute of ['aria-label', 'title']) labels.set(counter, 'Search failed', {}, attribute)
      }
    }
  }
}
on(document.querySelector('#search'), 'submit', event => { event.preventDefault(); void runSearch() })
on(document.getElementById('search-match-case'), 'change', () => {
  if (document.getElementById('query').value) void runSearch()
  else clearSearchResults()
})

function publicState() {
  const view = editor?.visible ? 'edit' : source?.visible ? 'source' : 'preview'
  const sourceAvailable = Boolean(index&&['markdown','csv','tsv','text','html'].includes(index.format))
  return { apiVersion: 1, bridgeVersion: 1, state: stopped ? 'disposed' : documentError ? 'error' : index ? formatLoading || formatRenderer?.state().ready === false ? 'rendering' : 'ready' : 'indexing', error: documentError,
    documentId, resourceId, revision: index?.revision ?? null, editingRevision: editor?.revision ?? null,
    presentation: presentation.state(), resources: documentReferences?.state() ?? null,
    timings: { indexMs: index?.indexMs ?? null, indexPeakRssKiB: index?.peakRssKib ?? null, firstUsefulContentMs: metrics.firstContentMs, maxHeartbeatGapMs: metrics.maxHeartbeatGapMs },
    editorError: editor?.error ? { code: editor.error.code || 'BACKEND', message: editor.error.message } : null,
    name: index?.name ?? null, format: index?.format ?? null, formatState: formatRenderer?.state() ?? null, view,
    document: index ? {fileBytes:index.originalBytes??index.sourceBytes,sourceBytes:sourceAvailable?index.sourceBytes:null,sourceLines:sourceAvailable?index.sourceLines:null,projectionPages:index.pages.length,tables:index.tables.length,assets:index.assets.length,references:index.references?.length??0} : null,
    outline: {open:!document.querySelector('#reader-outline').hidden,available:Boolean(index&&(formatRenderer||index.toc.length)),total:index?.renderer?formatOutlineTotal:index?.toc.length??0},
    dirty: Boolean(editor?.dirty), busy: Boolean(switching || editor?.busy || queryBusy || formatLoading || formatRenderer?.state().busy), needsRefresh: Boolean(editor?.needsRefresh),
    search: search.state(),
    capabilities: { ...serviceCapabilities, commentMarks: { version:1, hostReplacement:true, records:512 }, search: search.capabilities(), projectionWindow: Boolean(index && !formatRenderer), sourceRangeNavigation: sourceAvailable, sourceReading: sourceAvailable && !comparisonPreview ? source?.reading ?? null : null, passageNavigation: !comparisonPreview && (sourceAvailable || index?.renderer) ? { version: 1, focusHandoff: true, maxSegments: 32 } : null, presentation: { version: 1, locale: true, palette: true, fonts: true, notesOwner: true, selectionMenuOwner: true, textSelection: true }, charts: Boolean(serviceCapabilities.charts && index?.tables.length), edit: editable, source: sourceAvailable },
    positionPrecision: editor?.dirty ? 'draft' : formatRenderer?.state().format === 'epub' ? 'exact' : 'approximate',
    position: formatRenderer ? formatRenderer.state().position : index && tables && !editor?.dirty ? { version: 1, resourceId, revision: view === 'edit' ? editor.revision : index.revision, view, location: { kind: 'line', line: position().line } } : null }
}
async function publicSelection(signal, { textOnly = false } = {}) {
  if (!index) return null
  if (formatRenderer) { const value = formatRenderer.selection(); return textOnly && !value?.quote?.trim() ? null : value }
  if (source?.visible && source.external) {
    const value = source.selection()
    if (!value?.quote?.trim()) return null
    const quote = boundedQuote(value.quote)
    return { ...value, documentId, revision: index.revision, quote, quoteTruncated: Boolean(value.quoteTruncated || quote.length < value.quote.length) }
  }
  if (tables?.hasOverlay()) { const value = await tables.captureSelection({ textOnly }, signal); return value ? { documentId, revision: index.revision, ...value } : null }
  const code = editor?.visible ? editor.code : source?.visible ? source.code : null
  const range = getSelection()?.rangeCount ? getSelection().getRangeAt(0) : null
  const bounds = range?.getBoundingClientRect()
  const viewportRect = bounds ? { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } : null
  if (code) {
    // CodeMirror owns real text ranges even in a background WebKit frame,
    // where the window DOM Selection/focus may be absent.
    let selected = code.view.state.selection.main
    if (range && !range.collapsed && code.view.contentDOM.contains(range.startContainer) && code.view.contentDOM.contains(range.endContainer)) {
      const from = code.view.posAtDOM(range.startContainer, range.startOffset), to = code.view.posAtDOM(range.endContainer, range.endOffset)
      selected = { from: Math.min(from, to), to: Math.max(from, to), empty: from === to }
    }
    if (selected.empty) return null
    const codeRect = range && !range.collapsed && code.view.contentDOM.contains(range.startContainer) && code.view.contentDOM.contains(range.endContainer) ? viewportRect : null
    if (editor?.visible && editor.dirty) {
      const text = code.view.state.sliceDoc(selected.from, selected.to), quote = boundedQuote(text)
      return { documentId, revision: editor.revision, quote, quoteTruncated: quote.length < text.length,
        precision: 'draft', location: { kind: 'line', line: editor.line() }, viewportRect: codeRect }
    }
    if (editor?.visible && editor.needsRefresh) throw Error('Refresh the saved reading view before capturing an exact source selection')
    const page = editor?.visible ? editor.data.window : source.page
    const selection = await api('/api/selection/source', signal, { method: 'POST', body: JSON.stringify({ revision: index.revision,
      start: page.start, end: page.end, fromUtf16: selected.from, toUtf16: selected.to }) })
    if (stopped || !index || selection.documentId !== documentId || selection.revision !== index.revision) throw Object.assign(Error('Selection belongs to a previous reader revision'), { code: 'REVISION' })
    return { ...selection, viewportRect: codeRect }
  }
  if (!range || range.collapsed) { const value = await tables?.captureSelection({ textOnly }, signal); return value ? { documentId, revision: index.revision, ...value } : null }
  const element = node => node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement
  const start = element(range.startContainer), end = element(range.endContainer)
  const page = start?.closest('.page')
  if (!page || !space.contains(page) || !space.contains(end)) { const value = await tables?.captureSelection({ textOnly }, signal); return value ? { documentId, revision: index.revision, ...value } : null }
  const text = range.toString()
  if (textOnly && !text.trim()) return null
  const quote = boundedQuote(text), cell = start.closest('td,th')
  let location = { kind: 'line', line: Number(start.closest('[data-source-line]')?.dataset.sourceLine) || index.pages[Number(page.dataset.page)]?.line || 1 }, precision = 'approximate'
  if (cell?.tagName === 'TD' && cell === end.closest('td,th') && page.dataset.table != null) {
    const columns = JSON.parse(page.dataset.selectedColumns || '[]')
    location = { kind: 'cell', table: Number(page.dataset.table), row: Number(page.dataset.rowStart) + cell.parentElement.sectionRowIndex, column: columns[cell.cellIndex] }
    precision = 'exact'
  }
  return { documentId, revision: index.revision, quote, quoteTruncated: quote.length < text.length, precision, location, viewportRect, textRange: await captureProjectionComment(space, range, location, signal) }
}
function boundedQuote(text) {
  let end = 0, bytes = 0
  for (const scalar of text) {
    const point = scalar.codePointAt(0), size = point < 128 ? 1 : point < 2048 ? 2 : point < 65536 ? 3 : 4
    if (end + scalar.length > 16384 || bytes + size > 32768) break
    end += scalar.length; bytes += size
  }
  return text.slice(0, end)
}
async function navigateNote(note, { signal } = {}) {
  if (note.stale) throw Error('This note belongs to an earlier document revision')
  return navigateSavedPassage(note.anchor, note.revision, { signal })
}

async function navigateSavedPassage(anchor, revision, { signal } = {}) {
  signal?.throwIfAborted()
  if (!index || revision !== index.revision)
    throw Object.assign(Error('This saved passage belongs to an earlier document revision'), { code: 'REVISION' })
  const precision = validatePassage(anchor, index, formatRenderer?.state())
  passageRequest?.abort()
  passageFocus = null
  const controller = new AbortController()
  passageRequest = controller
  const signals = [controller.signal, readerAbort.signal]
  if (signal) signals.push(signal)
  try {
    return await revealSavedPassage(anchor, precision, AbortSignal.any(signals))
  } finally {
    if (passageRequest === controller) passageRequest = null
  }
}

async function revealSavedPassage({ location, textRange }, precision, signal) {
  const dirty = () => Object.assign(Error('Save your edits and refresh the reading view before opening a saved passage'), { code: 'DIRTY' })
  if (editor?.dirty || editor?.needsRefresh)
    throw dirty()
  // Notes can open while a preceding mode change finishes restoring its
  // bounded reading window. Let that operation settle before taking navigation
  // ownership, with cancellation and a finite wait instead of a spurious Save
  // instruction for a clean document.
  const waitingOwner = readerAbort, waitingIndex = index, waitingIntent = navigationIntent
  for (let attempt = 0; switching || editor?.busy; attempt++) {
    signal?.throwIfAborted()
    if (stopped || readerAbort !== waitingOwner || waitingOwner.signal.aborted || index !== waitingIndex || navigationIntent !== waitingIntent)
      throw new DOMException('Saved passage navigation was superseded', 'AbortError')
    if (attempt >= 150) throw Object.assign(Error('Reader is busy. Wait for the current operation to finish and try again.'), { code: 'BUSY' })
    await new Promise(resolve => setTimeout(resolve, 20))
  }
  signal?.throwIfAborted()
  if (stopped || readerAbort !== waitingOwner || waitingOwner.signal.aborted || index !== waitingIndex || navigationIntent !== waitingIntent)
    throw new DOMException('Saved passage navigation was superseded', 'AbortError')
  if (editor?.dirty || editor?.needsRefresh)
    throw dirty()
  const segment = textRange?.segments[0]
  locationRequest?.abort()
  const owner = readerAbort, snapshot = index, intent = ++navigationIntent
  const owned = () => !stopped && readerAbort === owner && !owner.signal.aborted && index === snapshot && navigationIntent === intent
  const current = () => {
    signal?.throwIfAborted()
    if (!owned()) throw new DOMException('Saved passage navigation was superseded', 'AbortError')
    return true
  }
  current()
  search.suspend()
  clearTextSelection()
  tables?.closeOverlays()
  let passage = null, fullCell = null, succeeded = false
  try {
    if (segment?.scope.kind === 'projection') {
      const page = segment.scope.page
      if (!Number.isSafeInteger(page) || !snapshot.pages[page] || snapshot.pages[page].table != null)
        throw Error('The saved passage page is unavailable')
      if (editor?.visible || source?.visible) {
        if (editable) await switchView('preview', { restorePreview: false })
        else await source.hide({ restore: false })
      }
      current()
      const ticket = await jump(page, null, null, null, null, '', {}, { intent, signal, keepMounted: true })
      try {
        current()
        if (ticket == null || ticket !== navigation) throw new DOMException('Navigation superseded', 'AbortError')
        const node = nodes.get(tables.itemForPage[page])
        passage = await resolveNotePassage(segment, [{ node, scope: segment.scope }], current)
        if (!passage) throw Object.assign(Error('The saved passage no longer matches the rendered text'), { code: 'PASSAGE_MISMATCH' })
        revealNotePassage(passage, viewport)
      } finally {
        if (ticket != null && ticket === navigation && readerAbort === owner && !owner.signal.aborted) { forcePage(null); schedule() }
      }
    } else if (segment?.scope.kind === 'cell') {
      const scope = segment.scope
      if (location?.kind !== 'cell' || ['table', 'row', 'column'].some(key => location[key] !== scope[key]))
        throw Error('The saved passage does not match its table cell')
      await navigateReadingLocation(location, { signal, intent, afterCell: async () => {
        passage = await resolveNotePassage(segment, commentScopes(shell, { viewport }), current)
        if (!passage) {
          fullCell = tables.openCell(scope.table)
          passage = await resolveNotePassage(segment, [{ node: fullCell.node, scope }], current)
        }
        if (!passage) throw Object.assign(Error('The saved passage no longer matches the rendered cell'), { code: 'PASSAGE_MISMATCH' })
        revealNotePassage(passage, fullCell?.node || viewport)
      } })
    } else await navigateReadingLocation(location, { signal, intent })
    current()
    succeeded = true
    commentMarks?.refresh()
    bridge?.publish('state', publicState())
    const sourceState = source?.visible ? source.code?.view.state : null
    // The Notes modal releases focus only after successful navigation. Restore
    // the exact selection then, without giving its former list row focus again.
    return { precision, focus(focusSignal) {
      const isCurrent = () => { focusSignal?.throwIfAborted(); return owned() && !editor?.visible }
      if (!isCurrent()) return false
      if (passage) {
        return focusNotePassage(passage, fullCell?.node || viewport, isCurrent)
      } else if (sourceState) {
        const code = source?.visible && source.code
        const state = code?.view.state
        if (state?.doc !== sourceState.doc || !state.selection.eq(sourceState.selection)) return false
        code.focus()
      } else if (formatRenderer?.focus) formatRenderer.focus()
      else viewport.focus({ preventScroll: true })
      return true
    } }
  } catch (error) { fullCell?.close(); throw error }
  // Source search uses the editor selection for its active match. A successful
  // note owns that selection until the next explicit search navigation.
  finally { if (owned() && !(succeeded && source?.visible)) search.repaint() }
}

async function navigateReadingLocation(location, { signal, intent, afterCell } = {}) {
  signal?.throwIfAborted()
  if (intent != null) return revealReadingLocation(location, { signal, intent, afterCell })
  const nextIntent = beginNavigation()
  const controller = new AbortController()
  locationRequest = controller
  const operation = signal ? AbortSignal.any([signal, controller.signal]) : controller.signal
  try {
    return await revealReadingLocation(location, { signal: operation, intent: nextIntent, afterCell })
  } finally {
    if (locationRequest === controller) locationRequest = null
  }
}

async function revealReadingLocation(location, { signal, intent, afterCell }) {
  signal?.throwIfAborted()
  if (!location || !index) throw Error('A reading location is required')
  const snapshot = index, lifetime = readerAbort
  const check = () => {
    signal?.throwIfAborted()
    if (stopped || index !== snapshot || readerAbort !== lifetime || lifetime.signal.aborted || intent !== navigationIntent)
      throw new DOMException('Navigation superseded', 'AbortError')
  }
  check()
  if (formatRenderer) { await formatRenderer.navigate(location, { signal }); check(); bridge?.publish('state', publicState()); return }
  if (location.kind === 'source') {
    if (!publicState().capabilities.source) throw Error('Source is unavailable for this format')
    const revision = index.revision
    if (editor?.visible) {
      // Leave clean Edit without restoring an unrelated Preview/Source position;
      // goRange below performs the single owned Source transition.
      await switchView('preview', { restorePreview: false })
      if (editor.visible) throw Error('Could not leave the current editing page')
    }
    check()
    if (index.revision !== revision) throw Object.assign(Error('Document revision changed while opening Source'), { code: 'REVISION' })
    await source.goRange(location.startByte,location.endByte, { signal, intent })
  } else if (location.kind === 'line') {
    if (!Number.isSafeInteger(location.line) || location.line < 1 || location.line > index.sourceLines) throw Error('Source line out of range')
    if (editor?.visible) { if (!await editor.go(location.line)) throw Error('Editor navigation failed') }
    else if (source?.visible) {
      if (!await source.go(location.line, { signal })) {
        check()
        throw Error('Could not navigate to this source line')
      }
    }
    else await previewLine(location, { intent, signal })
  } else if (location.kind === 'cell') {
    const table = Number.isSafeInteger(location.table) && location.table >= 0 ? index.tables[location.table] : null
    if (!table || !Number.isSafeInteger(location.row) || location.row < 0 || location.row >= table.rows || !Number.isSafeInteger(location.column) || location.column < 0 || location.column >= table.columns.length) throw Error('Table cell out of range')
    const revision = index.revision, owner = readerAbort, controller = tables
    const current = () => {
      signal?.throwIfAborted()
      if (stopped || owner.signal.aborted || readerAbort !== owner || index?.revision !== revision)
        throw Object.assign(Error('Document revision changed while opening Preview'), { code: 'REVISION' })
      if (intent !== navigationIntent) throw new DOMException('Navigation superseded', 'AbortError')
      return true
    }
    // The explicit target replaces Source's implicit position restoration.
    if (editor?.visible || source?.visible) {
      if (editable) await switchView('preview', { restorePreview: false })
      else await source.hide({ restore: false })
    }
    current()
    const ticket = await jump(table.firstPage, null, location.row, null, null, '', {}, { intent, signal, keepMounted: true })
    try {
      current()
      if (ticket == null || ticket !== navigation) throw new DOMException('Navigation superseded', 'AbortError')
      await controller.go(location.table, location.row, location.column, { signal, isCurrent: () => current() && ticket === navigation })
      current()
      await afterCell?.()
      current()
    } finally {
      if (ticket != null && ticket === navigation && readerAbort === owner && !owner.signal.aborted) { forcePage(null); schedule() }
    }
  } else throw Object.assign(Error('This renderer cannot resolve this location yet; retain the original location'), { code: 'UNSUPPORTED_LOCATION' })
  check()
  bridge?.publish('state', publicState())
}

function connectBridge() {
  const lifetime = new AbortController()
  let selectionTimer = null, selectionRequest = null, selectionGeneration = 0
  const result = publicReader({ origins: serviceCapabilities.parentOrigins || [], state: publicState, index: () => index, renderer: () => formatRenderer, api, navigateFragment, search,
    contents: showContents,
    async setView(view) {
      if (!index || editor?.busy || switching) throw Error('Reader is busy')
      if (view === 'edit' && !editable) throw Error('Editing was not granted')
      if (view !== 'preview' && !publicState().capabilities.source) throw Error('Source is unavailable for this format')
      search.suspend()
      clearTextSelection()
      if (editable) await switchView(view)
      else if (view === 'source') await source.show(position())
      else await source?.hide()
      search.repaint()
      if (publicState().view !== view) throw Error('Could not switch the reader view')
      result.publish('state', publicState())
    },
    navigate: navigateReadingLocation,
    async navigatePassage(anchor, signal) {
      if (!publicState().capabilities.passageNavigation) throw Object.assign(Error('Saved passage navigation is unavailable'), { code: 'CAPABILITY' })
      const result = await navigateSavedPassage(anchor, anchor.revision, { signal })
      signal.throwIfAborted()
      const focusToken = crypto.randomUUID()
      passageFocus = { focusToken, focus: result.focus }
      return { version: 1, documentId, revision: index.revision, view: publicState().view, precision: result.precision, focusToken }
    },
    async focusPassage(focusToken, signal) {
      const target = passageFocus
      if (!target || target.focusToken !== focusToken)
        throw Object.assign(Error('The saved passage focus has expired'), { code: 'STALE_FOCUS' })
      passageFocus = null
      if (!await target.focus(signal))
        throw Object.assign(Error('The saved passage focus has expired'), { code: 'STALE_FOCUS' })
      return { version: 1, focused: true }
    },
    selection: publicSelection,
    notesChanged: () => commentMarks?.refresh(true),
    setCommentMarks: marks => { if (presentation.state().notes !== 'host') throw Error('Host comment marks require host-owned Notes'); commentMarks?.replace(marks); return publicState() },
    async save() { if (!editable || !editor) throw Object.assign(Error('Editing was not granted'), { code: 'CAPABILITY' }); if (!await editor.save()) throw editor.error || Error('The editing page was not saved') },
    async settings(params) {
      const before = presentation.state()
      const typography = typographyAnchor()
      const after = presentation.update(params)
      syncWordHighlights()
      if (embedded && (params.notes !== undefined || params.selectionMenu !== undefined)) {
        const url = new URL(location.href)
        for (const key of ['notes', 'selectionMenu']) {
          if (params[key] === 'host') url.searchParams.set(key, 'host')
          else if (params[key] === 'reader') url.searchParams.delete(key)
        }
        history.replaceState(history.state, '', url)
      }
      if (before.notes !== presentation.state().notes || before.selectionMenu !== presentation.state().selectionMenu) {
        notes?.dispose(); notes = null
        syncNotesOwner()
      }
      if (params.theme != null) window.dispatchEvent(new CustomEvent('marknoria:theme-request', { detail: { theme: params.theme } }))
      if (params.wrap != null) document.getElementById('wrap').checked = after.wrap
      if (formatRenderer && params.pdf) await formatRenderer.settings(params.pdf)
      if (formatRenderer?.state().format === 'epub' && (params.epub || params.theme)) await formatRenderer.settings({ ...(params.theme ? { theme: params.theme } : {}), ...params.epub })
      if (layoutChanged(before, after)) await refreshTypography(typography)
      schedule()
    },
    async dispose() { search.dispose(); mainLifetime.abort(); presentation.dispose(); lifetime.abort(); shell.inert = true; window.readerExperiment = null; clearTimeout(selectionTimer); selectionRequest?.abort(); stopped = true; const closing = formatRenderer?.dispose(); teardown(); editor?.dispose(); files.dispose(); clearInterval(heartbeat); clearInterval(rendererHeartbeat); await closing },
  })
  document.addEventListener('selectionchange', () => {
    if (!result.connected) return
    clearTimeout(selectionTimer); const generation = ++selectionGeneration
    selectionRequest?.abort()
    selectionTimer = setTimeout(async () => {
      selectionRequest = new AbortController()
      try { const value = await publicSelection(selectionRequest.signal, { textOnly: true }); if (generation === selectionGeneration) result.publish('selection', value) } catch {}
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
  setStatus(error.message, null)
  files.state('error', error.message)
  document.querySelector('.reader-diagnostics').open = true
  }
  clearInterval(heartbeat)
}
