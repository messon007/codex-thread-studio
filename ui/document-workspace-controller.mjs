import { fuzzyFileLabel, reviewableFileKind } from './composer-tools.mjs'
import {
  artifactInlineSearchAvailable,
  artifactSearchAvailable,
  fileDisplayName,
  findTextMatchRanges,
  isHtmlFile,
  isMarkdownFile,
  STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  STATIC_HTML_FORBIDDEN_TAGS,
} from './document-review.mjs'
import {
  extractHtmlOutline,
  extractMarkdownOutline,
  filterDocumentOutline,
  normalizeDocumentOutline,
  outlineItemForLocation,
} from './document-outline.mjs'
import { epubCommentSource } from './epub-comment-provider.mjs'
import { pdfCommentSource } from './pdf-comment-provider.mjs'
import { tableCommentSource } from './table-comment-provider.mjs'
import DOMPurify from './vendor/purify.es.mjs'
import { t } from './i18n.mjs'

export function createDocumentWorkspaceState() {
  return {
    artifact: null,
    artifactView: 'preview',
    artifactSearch: '',
    artifactSearchMatches: [],
    artifactSearchIndex: -1,
    artifactSearchOpen: false,
    artifactOutlineOpen: false,
    artifactOutlineFilter: '',
    artifactOutlineActiveId: '',
    artifactOutlineCollapsed: new Set(),
  }
}

export function createDocumentWorkspaceController({
  state,
  gatewayFetch,
  view,
  randomId,
  notify = () => {},
  reportError = console.error,
}) {
  const {
    selectedThread,
    selectedStateKey,
    activateRightWorkspace,
    hideComposerMenu,
    closeActionMenus,
    applyRightRailWidth,
    renderMarkdownDocument,
    hydrateMarkdownImages,
    disconnectMarkdownImageObserver,
    disposeMarkdownImageAssets,
    hideSelection,
    setSelection,
    openWorkspaceTool,
    openResources,
    renderSessionMap,
    openBrowserUrl,
    readingTypography = () => ({}),
    reportClientError,
  } = view
  const $ = (selector) => document.querySelector(selector)
  const escapeHtml = (value) => String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  })[character])
  const toast = notify
  const showError = reportError
  let artifactSearchTimer = null
  let artifactOutlineLocationCleanup = null
  let artifactOutlineRefreshTimer = null
  let artifactOutlineIdleCallback = null
  let artifactEditor = null
  let epubReader = null
  let epubReaderModule = null
  let epubReaderGeneration = 0
  let epubReadingStateTimer = null
  let richArtifactReader = null
  let pdfReaderModule = null
  let tableReaderModule = null
  let workspaceEditorModule = null

  function bind() {
    $('#close-artifact')?.addEventListener('click', closeArtifactRail)
    $('#refresh-artifact')?.addEventListener('click', () => refreshArtifact().catch(showError))
    $('#artifact-preview')?.addEventListener('click', () => setArtifactView('preview'))
    $('#artifact-source')?.addEventListener('click', () => setArtifactView('source'))
    $('#artifact-edit')?.addEventListener('click', () => setArtifactView('edit'))
    $('#artifact-save')?.addEventListener('click', () => saveArtifact().catch(showError))
    $('#artifact-outline-toggle')?.addEventListener('click', toggleArtifactOutline)
    $('#artifact-outline-close')?.addEventListener('click', () => setArtifactOutlineOpen(false))
    $('#artifact-outline-backdrop')?.addEventListener('click', () => setArtifactOutlineOpen(false))
    $('#artifact-outline-filter')?.addEventListener('input', handleArtifactOutlineFilter)
    $('#artifact-outline-filter')?.addEventListener('keydown', handleArtifactOutlineFilterKeydown)
    $('#artifact-outline-list')?.addEventListener('click', (event) => handleArtifactOutlineClick(event).catch(showError))
    $('#artifact-search-toggle')?.addEventListener('click', toggleArtifactSearch)
    $('#artifact-search-input')?.addEventListener('input', handleArtifactSearchInput)
    $('#artifact-search-input')?.addEventListener('keydown', handleArtifactSearchKeydown)
    $('#artifact-search-prev')?.addEventListener('click', () => navigateArtifactSearch(-1))
    $('#artifact-search-next')?.addEventListener('click', () => navigateArtifactSearch(1))
    document.addEventListener('keydown', handleArtifactNavigationKeydown)
  }

async function openArtifact(file, { allowDetachedRoot = false, returnTool = '' } = {}) {
  const thread = selectedThread()
  if (!thread?.cwd && !allowDetachedRoot) throw new Error(t('The current session has no project directory, so the file cannot be opened safely.'))
  const root = String(file.root || thread?.cwd || '')
  if (!root) throw new Error(t('The current session has no project directory, so the file cannot be opened safely.'))
  const path = fuzzyFileLabel(file)
  const requestedEpubCfi = String(file.epubCfi || '')
  if (!path) throw new Error(t('The file path is empty.'))
  if (requestedEpubCfi && state.artifact?.kind === 'epub' && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    if (epubReader) await epubReader.display(requestedEpubCfi)
    else {
      state.artifact.readingState = { ...(state.artifact.readingState || {}), cfi: requestedEpubCfi }
      renderArtifact()
    }
    return
  }
  if (state.artifact?.dirty && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    return
  }
  if (state.artifact?.dirty && (state.artifact.root !== root || state.artifact.path !== path)) {
    if (!confirm(t('The current document has unsaved changes. Open another file anyway?'))) return
  }
  const kind = reviewableFileKind(file)
  if (!kind) throw new Error(t('This file type cannot be opened in the document reviewer'))
  resetArtifactSearch()
  resetArtifactOutline()
  activateRightWorkspace('document')
  hideComposerMenu()
  closeActionMenus()
  $('#artifact-content').classList.add('hidden')
  $('#artifact-search-panel').classList.add('hidden')
  $('#artifact-error').classList.add('hidden')
  $('#artifact-loading').classList.remove('hidden')
  disposeArtifactEditor()
  disposeEpubReader()
  disposeRichArtifactReader()
  disposeMarkdownImageAssets(state.artifact)
  const requestId = randomId()
  state.artifact = { root, path, kind, requestId, threadKey: selectedStateKey(), returnTool, loading: true }
  const endpoint = kind === 'image'
    ? '/studio/review-image'
    : kind === 'epub' ? '/studio/review-epub'
      : kind === 'pdf' ? '/studio/review-pdf'
        : kind === 'table' && /\.xlsx$/iu.test(path) ? '/studio/review-spreadsheet'
          : '/studio/review-file'
  const response = await gatewayFetch(endpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, path }),
  })
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    const message = result?.error?.message || `HTTP ${response.status}`
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = { ...state.artifact, loading: false, error: message }
    renderArtifact()
    throw new Error(message)
  }
  if (kind === 'image') {
    const blob = await response.blob()
    if (!blob.type.startsWith('image/')) {
      const message = t('The image response format is invalid')
      if (state.artifact?.requestId === requestId) {
        state.artifact = { ...state.artifact, loading: false, error: message }
        renderArtifact()
      }
      throw new Error(message)
    }
    const imageUrl = await blobToDataUrl(blob)
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) {
      return
    }
    state.artifact = {
      ...state.artifact,
      loading: false,
      mimeType: blob.type,
      imageUrl,
      relativePath: path,
      size: blob.size,
    }
    state.artifactView = 'image'
  } else if (kind === 'epub') {
    const bookHash = response.headers.get('x-studio-epub-hash') || ''
    const bytes = await response.arrayBuffer()
    if (!bookHash || !bytes.byteLength) {
      const message = t('Invalid EPUB response')
      if (state.artifact?.requestId === requestId) {
        state.artifact = { ...state.artifact, loading: false, error: message }
        renderArtifact()
      }
      throw new Error(message)
    }
    const readingState = await loadEpubReadingState(root, path, bookHash)
    if (requestedEpubCfi) readingState.cfi = requestedEpubCfi
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = {
      ...state.artifact,
      kind: 'epub',
      loading: false,
      bytes,
      hash: bookHash,
      relativePath: path,
      size: bytes.byteLength,
      readingState,
    }
    state.artifactView = 'epub'
  } else if (kind === 'pdf' || (kind === 'table' && /\.xlsx$/iu.test(path))) {
    const bytes = await response.arrayBuffer()
    if (!bytes.byteLength) throw new Error(t('Invalid document response'))
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = {
      ...state.artifact, kind, loading: false, bytes,
      hash: response.headers.get('x-studio-content-hash') || '',
      relativePath: path, size: bytes.byteLength,
    }
    state.artifactView = kind
  } else {
    const result = await response.json()
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = { ...result, kind: kind === 'table' ? 'table' : 'text', requestId, threadKey: selectedStateKey(), returnTool, loading: false }
    state.artifactView = kind === 'table' ? 'table' : isMarkdownFile(result.path) || isHtmlFile(result.path) ? 'preview' : 'source'
  }
  renderArtifact()
}

async function refreshArtifact() {
  if (!state.artifact) return
  if (state.artifact.dirty && !confirm(t('Reloading will discard unsaved changes. Continue?'))) return
  await openArtifact({ root: state.artifact.root, path: state.artifact.path }, { returnTool: state.artifact.returnTool })
}

function closeArtifactRail({ restoreMap = true, restoreWorkspace = true } = {}) {
  if (state.artifact?.dirty && !confirm(t('The current document has unsaved changes. Close it anyway?'))) return
  const returnTool = restoreWorkspace ? state.artifact?.returnTool : ''
  const artifactThreadKey = state.artifact?.threadKey
  $('#artifact-rail').classList.add('hidden')
  disposeArtifactEditor()
  disposeEpubReader()
  disposeRichArtifactReader()
  disposeMarkdownImageAssets(state.artifact)
  resetArtifactOutline()
  state.artifact = null
  resetArtifactSearch()
  hideSelection()
  if (returnTool && artifactThreadKey === selectedStateKey()) {
    if (returnTool === 'resources') {
      openResources()
      return
    }
    Promise.resolve(openWorkspaceTool(returnTool)).catch(showError)
    return
  }
  if (restoreMap && $('#annotation-rail').classList.contains('hidden') && $('#favorites-rail').classList.contains('hidden')) renderSessionMap()
}

function setArtifactView(view) {
  if (!state.artifact || state.artifact.kind !== 'text') return
  if (view === 'preview' && !isMarkdownFile(state.artifact.path) && !isHtmlFile(state.artifact.path)) return
  if (!['preview', 'source', 'edit'].includes(view)) return
  state.artifactView = view
  renderArtifact()
}

function toggleArtifactSearch() {
  if ($('#artifact-search-toggle').classList.contains('hidden')) return
  if (state.artifactView === 'edit') {
    artifactEditor?.openSearch?.()
    return
  }
  state.artifactSearchOpen = !state.artifactSearchOpen
  $('#artifact-search-toggle').classList.toggle('active', state.artifactSearchOpen)
  $('#artifact-search-toggle').setAttribute('aria-expanded', String(state.artifactSearchOpen))
  $('#artifact-search-panel').classList.toggle('hidden', !state.artifactSearchOpen)
  if (state.artifactSearchOpen) requestAnimationFrame(() => $('#artifact-search-input').focus())
}

function toggleArtifactOutline() {
  if ($('#artifact-outline-toggle').classList.contains('hidden')) return
  setArtifactOutlineOpen(!state.artifactOutlineOpen)
}

function setArtifactOutlineOpen(open) {
  state.artifactOutlineOpen = Boolean(open)
  renderArtifactOutline()
  if (state.artifactOutlineOpen) requestAnimationFrame(() => $('#artifact-outline-filter').focus())
}

function handleArtifactNavigationKeydown(event) {
  if (event.key !== 'Escape' || $('#artifact-rail').classList.contains('hidden')) return
  if (state.artifactOutlineOpen && !$('#artifact-outline-toggle').classList.contains('hidden')) {
    event.preventDefault()
    setArtifactOutlineOpen(false)
  } else if (state.artifactSearchOpen) {
    event.preventDefault()
    state.artifactSearchOpen = false
    $('#artifact-search-panel').classList.add('hidden')
    $('#artifact-search-toggle').classList.remove('active')
    $('#artifact-search-toggle').setAttribute('aria-expanded', 'false')
  }
}

function resetArtifactOutline() {
  disposeArtifactOutlineBindings()
  cancelArtifactOutlineWork()
  state.artifactOutlineFilter = ''
  state.artifactOutlineActiveId = ''
  state.artifactOutlineCollapsed = new Set()
  state.artifactOutlineOpen = false
  const filter = $('#artifact-outline-filter')
  if (filter) filter.value = ''
  $('#artifact-outline-list')?.replaceChildren()
  $('#artifact-outline')?.classList.add('hidden')
  $('#artifact-outline-toggle')?.classList.add('hidden')
  $('#artifact-reader-shell')?.classList.remove('outline-open')
}

function setArtifactOutline(file, items, provider = file?.outlineProvider || null) {
  if (!file || state.artifact !== file) return
  file.outlineItems = normalizeDocumentOutline(items)
  file.outlineProvider = provider
  if (state.artifactOutlineActiveId && !file.outlineItems.some((item) => item.id === state.artifactOutlineActiveId)) {
    state.artifactOutlineActiveId = ''
  }
  renderArtifactOutline()
}

function renderArtifactOutline() {
  const file = state.artifact
  const items = file?.outlineItems || []
  const available = items.length > 0
  const toggle = $('#artifact-outline-toggle')
  const outline = $('#artifact-outline')
  const shell = $('#artifact-reader-shell')
  toggle.classList.toggle('hidden', !available)
  toggle.classList.toggle('active', available && state.artifactOutlineOpen)
  toggle.setAttribute('aria-expanded', String(available && state.artifactOutlineOpen))
  outline.classList.toggle('hidden', !available || !state.artifactOutlineOpen)
  shell.classList.toggle('outline-open', available && state.artifactOutlineOpen)
  $('#artifact-outline-count').textContent = String(items.length)
  if (!available) {
    $('#artifact-outline-list').replaceChildren()
    return
  }
  const filtered = filterDocumentOutline(items, state.artifactOutlineFilter)
  const byId = new Map(items.map((item) => [item.id, item]))
  const visible = state.artifactOutlineFilter.trim() ? filtered : filtered.filter((item) => {
    let parentId = item.parentId
    while (parentId) {
      if (state.artifactOutlineCollapsed.has(parentId)) return false
      parentId = byId.get(parentId)?.parentId || ''
    }
    return true
  })
  const childParents = new Set(items.map((item) => item.parentId).filter(Boolean))
  const list = $('#artifact-outline-list')
  list.innerHTML = visible.length ? visible.map((item) => {
    const hasChildren = childParents.has(item.id)
    const collapsed = state.artifactOutlineCollapsed.has(item.id)
    const depth = Math.min(4, item.depth)
    return `<button class="artifact-outline-row${item.id === state.artifactOutlineActiveId ? ' active' : ''}${item.contextOnly ? ' context-only' : ''}${collapsed ? ' collapsed' : ''}" style="--outline-depth:${depth}" type="button" data-outline-id="${escapeHtml(item.id)}" title="${escapeHtml(item.label)}"><span class="artifact-outline-chevron"${hasChildren ? ' data-outline-collapse="true"' : ''}>${hasChildren ? '⌄' : ''}</span><span class="artifact-outline-label">${escapeHtml(item.label)}</span></button>`
  }).join('') : `<p class="artifact-outline-empty">${escapeHtml(t('No matching sections'))}</p>`
  scrollActiveOutlineItemIntoView()
}

function handleArtifactOutlineFilter(event) {
  state.artifactOutlineFilter = event.target.value
  renderArtifactOutline()
}

async function handleArtifactOutlineFilterKeydown(event) {
  if (event.key !== 'Enter') return
  const first = $('#artifact-outline-list .artifact-outline-row:not(.context-only)')
  if (!first) return
  event.preventDefault()
  await navigateArtifactOutlineItem(first.dataset.outlineId)
}

async function handleArtifactOutlineClick(event) {
  const row = event.target.closest('[data-outline-id]')
  if (!row) return
  const id = row.dataset.outlineId
  if (event.target.closest('[data-outline-collapse]')) {
    if (state.artifactOutlineCollapsed.has(id)) state.artifactOutlineCollapsed.delete(id)
    else state.artifactOutlineCollapsed.add(id)
    renderArtifactOutline()
    return
  }
  await navigateArtifactOutlineItem(id)
}

async function navigateArtifactOutlineItem(id) {
  const file = state.artifact
  const item = file?.outlineItems?.find((candidate) => candidate.id === id)
  if (!item) return
  setArtifactOutlineActive(id)
  await file.outlineProvider?.navigate?.(item)
}

function setArtifactOutlineActive(id) {
  if (!id || state.artifactOutlineActiveId === id) return
  state.artifactOutlineActiveId = id
  $('#artifact-outline-list').querySelectorAll('[data-outline-id]').forEach((row) => row.classList.toggle('active', row.dataset.outlineId === id))
  scrollActiveOutlineItemIntoView()
}

function scrollActiveOutlineItemIntoView() {
  const active = [...$('#artifact-outline-list').querySelectorAll('[data-outline-id]')].find((row) => row.dataset.outlineId === state.artifactOutlineActiveId)
  if (!active) return
  const list = $('#artifact-outline-list')
  const listRect = list.getBoundingClientRect()
  const rowRect = active.getBoundingClientRect()
  if (rowRect.top < listRect.top) list.scrollTop -= listRect.top - rowRect.top
  else if (rowRect.bottom > listRect.bottom) list.scrollTop += rowRect.bottom - listRect.bottom
}

function scheduleTextArtifactOutline(file, content, source, options) {
  cancelArtifactOutlineWork()
  const run = () => {
    artifactOutlineIdleCallback = null
    if (state.artifact !== file || !content.isConnected) return
    configureTextArtifactOutline(file, content, source, options)
  }
  artifactOutlineRefreshTimer = setTimeout(() => {
    artifactOutlineRefreshTimer = null
    if (globalThis.requestIdleCallback) {
      artifactOutlineIdleCallback = requestIdleCallback(run, { timeout: 400 })
    } else {
      run()
    }
  }, 0)
}

function cancelArtifactOutlineWork() {
  clearTimeout(artifactOutlineRefreshTimer)
  artifactOutlineRefreshTimer = null
  if (artifactOutlineIdleCallback != null && globalThis.cancelIdleCallback) {
    cancelIdleCallback(artifactOutlineIdleCallback)
  }
  artifactOutlineIdleCallback = null
}

function configureTextArtifactOutline(file, content, source, { markdown, html, view, items: preparedItems = null }) {
  if (!markdown && !html) {
    setArtifactOutline(file, [])
    return
  }
  let items = Array.isArray(preparedItems)
    ? preparedItems
    : markdown ? extractMarkdownOutline(source) : extractHtmlOutline(source)
  if (view === 'preview') {
    const headings = [...content.querySelectorAll('h1, h2, h3, h4, h5, h6')]
    if (html) {
      items = normalizeDocumentOutline(headings.slice(0, 2_000).map((heading, index) => ({
        ...(items[index] || {}),
        id: items[index]?.id || `section-${index + 1}`,
        label: heading.textContent.trim() || t('Untitled section'),
        depth: Number(heading.tagName.slice(1)) - 1,
        target: items[index]?.target || { kind: 'text-heading', offset: 0, line: 1 },
      })))
    }
    headings.forEach((heading, index) => {
      if (items[index]) heading.dataset.documentOutlineId = items[index].id
    })
  }
  const provider = { navigate: (item) => navigateTextArtifactOutline(file, content, item, view) }
  setArtifactOutline(file, items, provider)
  if (view === 'preview' && items.length) bindTextArtifactOutlineLocation(content)
  else if (items.length && !state.artifactOutlineActiveId) setArtifactOutlineActive(items[0].id)
}

function bindTextArtifactOutlineLocation(content) {
  const headings = [...content.querySelectorAll('[data-document-outline-id]')]
  if (!headings.length) return
  let frame = null
  const sync = () => {
    frame = null
    const threshold = content.getBoundingClientRect().top + 42
    let active = headings[0]
    for (const heading of headings) {
      if (heading.getBoundingClientRect().top > threshold) break
      active = heading
    }
    setArtifactOutlineActive(active.dataset.documentOutlineId)
  }
  const onScroll = () => {
    if (frame != null) return
    frame = requestAnimationFrame(sync)
  }
  content.addEventListener('scroll', onScroll, { passive: true })
  artifactOutlineLocationCleanup = () => {
    content.removeEventListener('scroll', onScroll)
    if (frame != null) cancelAnimationFrame(frame)
  }
  sync()
}

async function navigateTextArtifactOutline(file, content, item, view) {
  if (state.artifact !== file) return
  if (view === 'preview') {
    const heading = [...content.querySelectorAll('[data-document-outline-id]')].find((candidate) => candidate.dataset.documentOutlineId === item.id)
    if (!heading) return
    const targetTop = content.scrollTop + heading.getBoundingClientRect().top - content.getBoundingClientRect().top - 18
    content.scrollTo({ top: Math.max(0, targetTop), behavior: 'smooth' })
    flashArtifactOutlineTarget(heading)
    return
  }
  if (view === 'edit') {
    artifactEditor?.revealOffset?.(item.target?.offset || 0)
    return
  }
  const pre = content.querySelector('.artifact-source')
  const textNode = pre?.firstChild
  if (!textNode) return
  const offset = Math.min(textNode.textContent.length, Math.max(0, Number(item.target?.offset) || 0))
  const range = document.createRange()
  range.setStart(textNode, offset)
  range.setEnd(textNode, offset)
  const rect = range.getBoundingClientRect()
  const contentRect = content.getBoundingClientRect()
  content.scrollTo({ top: Math.max(0, content.scrollTop + rect.top - contentRect.top - 18), behavior: 'auto' })
  flashArtifactOutlineTarget(pre)
}

function flashArtifactOutlineTarget(element) {
  element.classList.remove('artifact-outline-target')
  void element.offsetWidth
  element.classList.add('artifact-outline-target')
  setTimeout(() => element.classList.remove('artifact-outline-target'), 1_300)
}

async function saveArtifact({ overwrite = false } = {}) {
  const file = state.artifact
  if (!file || state.artifactView !== 'edit' || !file.dirty) return
  const content = artifactEditor?.value() ?? file.editContent ?? file.content
  file.saving = true
  $('#artifact-save').disabled = true
  const response = await gatewayFetch('/studio/workspace/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root: file.root, path: file.path, content, expectedHash: file.hash, overwrite }),
  })
  const result = await response.json().catch(() => null)
  if (response.status === 409 && result?.error?.code === 'workspace_file_conflict') {
    file.saving = false
    $('#artifact-save').disabled = false
    if (confirm(t('The file changed on disk. Overwrite it with the current editor content?'))) {
      await saveArtifact({ overwrite: true })
    }
    return
  }
  if (!response.ok) {
    file.saving = false
    $('#artifact-save').disabled = false
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
  state.artifact = {
    ...result,
    kind: 'text',
    requestId: file.requestId,
    threadKey: file.threadKey,
    returnTool: file.returnTool,
    loading: false,
    dirty: false,
    editContent: result.content,
  }
  renderArtifact()
  toast('File saved')
}

function renderArtifact() {
  const rail = $('#artifact-rail')
  const file = state.artifact
  disconnectMarkdownImageObserver()
  if (!file) {
    disposeArtifactEditor()
    disposeArtifactOutlineBindings()
    rail.classList.add('hidden')
    resetArtifactSearch()
    return
  }
  disposeArtifactEditor()
  disposeArtifactOutlineBindings()
  disposeEpubReader()
  applyRightRailWidth()
  rail.classList.remove('hidden')
  $('#artifact-title').textContent = fileDisplayName(file.path)
  $('#artifact-path').textContent = file.relativePath || file.path
  const closeButton = $('#close-artifact')
  const returnLabel = file.returnTool === 'files' ? t('Back to Files') : file.returnTool === 'review' ? t('Back to Git Review') : file.returnTool === 'resources' ? t('Back to Resources') : t('Close document')
  closeButton.classList.toggle('returning', Boolean(file.returnTool))
  closeButton.title = returnLabel
  closeButton.setAttribute('aria-label', returnLabel)
  $('#artifact-loading').classList.toggle('hidden', !file.loading)
  $('#artifact-error').classList.toggle('hidden', !file.error)
  $('#artifact-error-message').textContent = file.error || ''
  const textReady = !file.loading && !file.error && file.kind === 'text' && typeof file.content === 'string'
  const imageReady = !file.loading && !file.error && file.kind === 'image' && Boolean(file.imageUrl)
  const epubReady = !file.loading && !file.error && file.kind === 'epub' && file.bytes instanceof ArrayBuffer
  const pdfReady = !file.loading && !file.error && file.kind === 'pdf' && file.bytes instanceof ArrayBuffer
  const tableReady = !file.loading && !file.error && file.kind === 'table' && (file.bytes instanceof ArrayBuffer || typeof file.content === 'string')
  const ready = textReady || imageReady || epubReady || pdfReady || tableReady
  const content = $('#artifact-content')
  $('#artifact-reader-shell').classList.toggle('hidden', !ready)
  content.classList.toggle('hidden', !ready)
  $('#artifact-meta').textContent = textReady
    ? t('{lines} lines · {size}', { lines: file.lineCount, size: formatFileSize(file.size) })
    : imageReady ? `${file.mimeType.replace('image/', '').toUpperCase()} · ${formatFileSize(file.size)}`
      : epubReady ? `EPUB · ${formatFileSize(file.size)}`
        : pdfReady ? `PDF · ${formatFileSize(file.size)}`
          : tableReady ? `${/\.xlsx$/iu.test(file.path) ? 'XLSX' : 'CSV'} · ${formatFileSize(file.size)}` : ''
  $('#artifact-hint').textContent = t(file.kind === 'image' ? 'Image previews do not support comments' : file.kind === 'epub' ? 'Select book text, add a question, and send it to AI' : file.kind === 'pdf' ? 'Select PDF text or Shift-drag a region to comment' : file.kind === 'table' ? 'Select a cell to comment' : 'Select text to comment')
  const markdown = textReady && isMarkdownFile(file.path)
  const html = textReady && isHtmlFile(file.path)
  const editable = textReady
  const renderedContent = file.editContent ?? file.content
  $('#artifact-title').textContent = `${fileDisplayName(file.path)}${file.dirty ? ' •' : ''}`
  $('#artifact-view-switch').classList.toggle('hidden', !editable)
  $('#artifact-preview').disabled = !markdown && !html
  $('#artifact-preview').classList.toggle('active', state.artifactView === 'preview')
  $('#artifact-source').classList.toggle('active', state.artifactView === 'source')
  $('#artifact-edit').classList.toggle('active', state.artifactView === 'edit')
  $('#artifact-save').classList.toggle('hidden', state.artifactView !== 'edit')
  $('#artifact-save').disabled = !file.dirty || Boolean(file.saving)
  const canSearch = artifactSearchAvailable(file, state.artifactView)
  const canInlineSearch = artifactInlineSearchAvailable(file, state.artifactView)
  if (!canInlineSearch) state.artifactSearchOpen = false
  $('#artifact-search-toggle').classList.toggle('hidden', !canSearch)
  $('#artifact-search-toggle').classList.toggle('active', canInlineSearch && state.artifactSearchOpen)
  $('#artifact-search-toggle').setAttribute('aria-expanded', String(canInlineSearch && state.artifactSearchOpen))
  $('#artifact-search-panel').classList.toggle('hidden', !canInlineSearch || !state.artifactSearchOpen)
  $('#artifact-search-input').setAttribute('placeholder', t('Search document content…'))
  $('#artifact-search-prev').title = t('Previous match')
  $('#artifact-search-prev').setAttribute('aria-label', t('Previous match'))
  $('#artifact-search-next').title = t('Next match')
  $('#artifact-search-next').setAttribute('aria-label', t('Next match'))
  if (!ready) {
    setArtifactOutline(file, [])
    return
  }
  if (imageReady) {
    setArtifactOutline(file, [])
    content.className = 'artifact-content artifact-image-preview'
    content.innerHTML = `<div class="artifact-image-stage"><img src="${escapeHtml(file.imageUrl)}" alt="${escapeHtml(fileDisplayName(file.path))}" draggable="false" /></div>`
    const image = content.querySelector('img')
    image?.addEventListener('load', () => {
      if (state.artifact?.requestId !== file.requestId) return
      $('#artifact-meta').textContent = `${image.naturalWidth} × ${image.naturalHeight} · ${file.mimeType.replace('image/', '').toUpperCase()} · ${formatFileSize(file.size)}`
    }, { once: true })
    image?.addEventListener('error', () => {
      if (state.artifact?.requestId !== file.requestId) return
      state.artifact = { ...state.artifact, error: t('Unable to decode image') }
      renderArtifact()
    }, { once: true })
    renderArtifactSearchStatus()
    return
  }
  if (epubReady) {
    setArtifactOutline(file, file.outlineItems || [])
    content.className = 'artifact-content artifact-epub-preview'
    content.innerHTML = '<div class="artifact-epub-host" data-no-i18n></div>'
    mountEpubReader(file, content.firstElementChild).catch((error) => {
      if (state.artifact?.requestId !== file.requestId) return
      state.artifact = { ...state.artifact, error: error.message || String(error) }
      renderArtifact()
    })
    renderArtifactSearchStatus()
    return
  }
  if (pdfReady) {
    setArtifactOutline(file, file.outlineItems || [])
    content.className = 'artifact-content artifact-pdf-preview'
    content.innerHTML = '<div class="artifact-pdf-host" data-no-i18n></div>'
    mountPdfReader(file, content.firstElementChild).catch(showError)
    renderArtifactSearchStatus()
    return
  }
  if (tableReady) {
    setArtifactOutline(file, [])
    content.className = 'artifact-content artifact-table-preview'
    content.innerHTML = '<div class="artifact-table-host" data-no-i18n></div>'
    mountTableReader(file, content.firstElementChild).catch(showError)
    renderArtifactSearchStatus()
    return
  }
  if (state.artifactView === 'edit') {
    content.className = 'artifact-content editing'
    content.innerHTML = '<div class="artifact-editor-shell" data-no-i18n></div>'
    mountArtifactEditor(file, content.firstElementChild, renderedContent).catch(showError)
  } else if (markdown && state.artifactView === 'preview') {
    const rendered = renderMarkdownDocument(renderedContent)
    content.className = 'artifact-content markdown-body'
    content.innerHTML = rendered.html
    hydrateMarkdownImages(file, content)
    scheduleTextArtifactOutline(file, content, renderedContent, {
      markdown, html, view: state.artifactView, items: rendered.outline,
    })
  } else if (html && state.artifactView === 'preview') {
    content.className = 'artifact-content markdown-body artifact-html-preview'
    content.innerHTML = renderStaticHtml(renderedContent)
  } else {
    content.className = 'artifact-content'
    content.innerHTML = `<pre class="artifact-source" data-no-i18n>${escapeHtml(renderedContent)}</pre>`
  }
  if (!(markdown && state.artifactView === 'preview')) {
    configureTextArtifactOutline(file, content, renderedContent, { markdown, html, view: state.artifactView })
  }
  if (state.artifactSearch) applyArtifactSearchHighlights()
  else renderArtifactSearchStatus()
}

function disposeArtifactEditor() {
  artifactEditor?.destroy()
  artifactEditor = null
}

function disposeArtifactOutlineBindings() {
  artifactOutlineLocationCleanup?.()
  artifactOutlineLocationCleanup = null
  cancelArtifactOutlineWork()
}

function disposeEpubReader() {
  epubReaderGeneration += 1
  clearTimeout(epubReadingStateTimer)
  epubReadingStateTimer = null
  if (state.artifact?.kind === 'epub' && state.artifact.readingState) {
    persistEpubReadingState(state.artifact).catch(() => {})
  }
  epubReader?.destroy()
  epubReader = null
}

function disposeRichArtifactReader() {
  artifactOutlineLocationCleanup?.()
  artifactOutlineLocationCleanup = null
  richArtifactReader?.destroy?.()
  richArtifactReader = null
}

async function mountPdfReader(file, parent) {
  pdfReaderModule ||= import('./pdf-reader.mjs')
  const { createPdfReader } = await pdfReaderModule
  if (state.artifact !== file || !parent.isConnected) return
  const reader = await createPdfReader({
    container: parent, bytes: file.bytes, initialPage: file.page || 1, search: state.artifactSearch,
    translate: t,
    onPageChange: (page) => {
      file.page = page
      const active = outlineItemForLocation(file.outlineItems, { page })
      if (active) setArtifactOutlineActive(active.id)
    },
    onSelection: (selection) => {
      if (state.artifact !== file || !selection.quote) return
      setSelection(
        { quote: selection.quote, itemId: null, turnId: null, source: pdfCommentSource({ root: file.root, filePath: file.path, documentHash: file.hash, page: selection.page, rects: selection.rects }) },
        selection.rect,
      )
    },
  })
  if (state.artifact !== file || !parent.isConnected) {
    reader.destroy()
    return
  }
  richArtifactReader = reader
  const outline = reader.outline()
  setArtifactOutline(file, outline, { navigate: (item) => reader.goToPage(item.target?.page) })
  const active = outlineItemForLocation(outline, { page: file.page || 1 })
  if (active) setArtifactOutlineActive(active.id)
}

async function mountTableReader(file, parent) {
  tableReaderModule ||= import('./table-reader.mjs')
  const { parseTabularArtifact, renderTableArtifact } = await tableReaderModule
  if (!file.workbook) file.workbook = await parseTabularArtifact({ bytes: file.bytes, path: file.path, text: file.content })
  if (state.artifact !== file || !parent.isConnected) return
  richArtifactReader = renderTableArtifact({
    container: parent, workbook: file.workbook, translate: t,
    onSelection: (selection) => {
      if (state.artifact !== file) return
      setSelection(
        { quote: selection.quote || `[${selection.range}]`, itemId: null, turnId: null, source: tableCommentSource({ root: file.root, filePath: file.path, documentHash: file.hash, sheet: selection.sheet, range: selection.range }) },
        selection.rect,
      )
    },
  })
}

async function loadEpubReadingState(root, path, bookHash) {
  const response = await gatewayFetch('/studio/epub/state', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, path, bookHash }),
  })
  if (response.status === 204) return {}
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
  return response.json()
}

async function mountEpubReader(file, parent) {
  const generation = ++epubReaderGeneration
  epubReaderModule ||= import('./epub-reader.mjs')
  const { createEpubReader } = await epubReaderModule
  if (generation !== epubReaderGeneration || state.artifact !== file || !parent.isConnected) return
  const reader = await createEpubReader({
    container: parent,
    bytes: file.bytes,
    initialState: file.readingState,
    typography: readingTypography(),
    translate: t,
    onSelection: (selection) => captureEpubSelection(file, selection),
    onRelocate: (readingState) => {
      if (state.artifact !== file) return
      file.readingState = readingState
      const active = outlineItemForLocation(file.outlineItems, readingState.href)
      if (active) setArtifactOutlineActive(active.id)
      scheduleEpubReadingState(file)
    },
    onExternalLink: (url) => openBrowserUrl(url).catch(showError),
  })
  if (generation !== epubReaderGeneration || state.artifact !== file || !parent.isConnected) {
    reader.destroy()
    return
  }
  epubReader = reader
  const outline = reader.outline()
  setArtifactOutline(file, outline, { navigate: (item) => reader.display(item.target?.href) })
  const active = outlineItemForLocation(outline, reader.state().href)
  if (active) setArtifactOutlineActive(active.id)
  file.bookTitle = reader.title
  if (file.bookTitle) $('#artifact-title').textContent = file.bookTitle
}

function scheduleEpubReadingState(file) {
  clearTimeout(epubReadingStateTimer)
  epubReadingStateTimer = setTimeout(() => {
    epubReadingStateTimer = null
    persistEpubReadingState(file).catch((error) => reportClientError(new Error(`EPUB reading state: ${error?.message || error}`)))
  }, 450)
}

async function persistEpubReadingState(file) {
  if (!file?.readingState || !file.hash) return
  const reading = file.readingState
  const response = await gatewayFetch('/studio/epub/state', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      root: file.root,
      path: file.path,
      bookHash: file.hash,
      cfi: reading.cfi || '',
      chapterLabel: reading.chapterLabel || '',
      progress: Number(reading.progress) || 0,
      fontScale: Number(reading.fontScale) || 1,
      theme: reading.theme || 'light',
      flow: reading.flow || 'paginated',
      tocOpen: Boolean(reading.tocOpen),
    }),
  })
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
}

function captureEpubSelection(file, selection) {
  if (state.artifact !== file || !selection?.quote) return
  setSelection({
    quote: selection.quote,
    itemId: null,
    turnId: null,
    source: epubCommentSource({
      root: file.root,
      filePath: file.path,
      bookHash: file.hash,
      cfiRange: selection.cfiRange,
      href: selection.href,
      chapterLabel: selection.chapterLabel,
    }),
  }, selection.rect)
}

async function reopenEpubComment(anchor) {
  await openArtifact({
    root: anchor.root || selectedThread()?.cwd,
    path: anchor.filePath,
    epubCfi: anchor.cfiRange,
  })
}

async function mountArtifactEditor(file, parent, content) {
  workspaceEditorModule ||= import('./workspace-editor.mjs')
  const { createWorkspaceEditor } = await workspaceEditorModule
  if (state.artifact !== file || state.artifactView !== 'edit' || !parent.isConnected) return
  artifactEditor = createWorkspaceEditor({
    parent,
    content,
    language: file.language,
    onSave: () => saveArtifact().catch(showError),
    onChange: (value) => {
      file.editContent = value
      file.dirty = file.editContent !== file.content
      const lines = file.editContent ? file.editContent.split('\n').length : 1
      $('#artifact-title').textContent = `${fileDisplayName(file.path)}${file.dirty ? ' •' : ''}`
      $('#artifact-save').disabled = !file.dirty
      $('#artifact-meta').textContent = t('{lines} lines · {size}', { lines, size: formatFileSize(new TextEncoder().encode(file.editContent).length) })
      if (isMarkdownFile(file.path) || isHtmlFile(file.path)) {
        cancelArtifactOutlineWork()
        artifactOutlineRefreshTimer = setTimeout(() => {
          artifactOutlineRefreshTimer = null
          if (state.artifact !== file || state.artifactView !== 'edit') return
          configureTextArtifactOutline(file, $('#artifact-content'), file.editContent, {
            markdown: isMarkdownFile(file.path),
            html: isHtmlFile(file.path),
            view: 'edit',
          })
        }, 250)
      }
    },
  })
  artifactEditor.focus()
}

function renderStaticHtml(value) {
  const clean = DOMPurify.sanitize(String(value || ''), {
    USE_PROFILES: { html: true },
    FORBID_TAGS: STATIC_HTML_FORBIDDEN_TAGS,
    FORBID_ATTR: STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  })
  const template = document.createElement('template')
  template.innerHTML = clean
  template.content.querySelectorAll('*').forEach((element) => {
    for (const attribute of [...element.attributes]) {
      if (/^on/iu.test(attribute.name)) element.removeAttribute(attribute.name)
    }
  })
  return template.innerHTML
}

function blobToDataUrl(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.addEventListener('load', () => resolve(String(reader.result || '')), { once: true })
    reader.addEventListener('error', () => reject(reader.error || new Error(t('Unable to decode image'))), { once: true })
    reader.readAsDataURL(blob)
  })
}

function resetArtifactSearch() {
  state.artifactSearch = ''
  state.artifactSearchIndex = -1
  state.artifactSearchMatches = []
  state.artifactSearchOpen = false
  const searchInput = $('#artifact-search-input')
  if (searchInput) searchInput.value = ''
  $('#artifact-search-toolbar')?.classList.remove('hidden')
  $('#artifact-search-panel')?.classList.add('hidden')
  $('#artifact-search-toggle')?.classList.remove('active')
  $('#artifact-search-toggle')?.setAttribute('aria-expanded', 'false')
  clearArtifactSearchHighlights()
  renderArtifactSearchStatus()
  clearTimeout(artifactSearchTimer)
}

function handleArtifactSearchInput(event) {
  state.artifactSearch = event.target.value
  clearTimeout(artifactSearchTimer)
  if (!state.artifact || !state.artifactSearch.trim()) {
    clearArtifactSearchHighlights()
    renderArtifactSearchStatus()
    return
  }
  artifactSearchTimer = setTimeout(() => {
    const content = $('#artifact-content')
    if (!content || content.classList.contains('hidden')) return
    applyArtifactSearchHighlights()
  }, 140)
}

function handleArtifactSearchKeydown(event) {
  if (event.key !== 'Enter') return
  event.preventDefault()
  if (!state.artifactSearch.trim() || state.artifactSearchMatches.length === 0) return
  navigateArtifactSearch(event.shiftKey ? -1 : 1)
}

function handleArtifactSearchMatches(raw = '') {
  const content = $('#artifact-content')
  if (!content || !raw.trim()) return []
  const query = raw.trim()
  const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue) return NodeFilter.FILTER_SKIP
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (parent.closest('button, .markdown-code-header, .markdown-mermaid-canvas, script, style, textarea, .artifact-search-highlight')) {
        return NodeFilter.FILTER_REJECT
      }
      const mermaidSource = parent.closest('.markdown-mermaid-source')
      if (mermaidSource && !mermaidSource.closest('.markdown-mermaid.show-source')) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })

  const matches = []
  while (walker.nextNode()) {
    const node = walker.currentNode
    findTextMatchRanges(node.nodeValue, query).forEach(({ start, end }) => {
      matches.push({ node, start, end })
    })
  }
  return matches
}

function clearArtifactSearchHighlights() {
  const content = $('#artifact-content')
  if (!content) return
  content.querySelectorAll('mark.artifact-search-highlight').forEach((mark) => {
    const parent = mark.parentElement
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  })
  state.artifactSearchMatches = []
  state.artifactSearchIndex = -1
}

function applyArtifactSearchHighlights() {
  const content = $('#artifact-content')
  if (!content || !artifactInlineSearchAvailable(state.artifact, state.artifactView)) {
    clearArtifactSearchHighlights()
    return
  }
  clearArtifactSearchHighlights()
  const query = state.artifactSearch.trim()
  if (!query) {
    renderArtifactSearchStatus()
    return
  }

  const nodeGroups = new Map()
  handleArtifactSearchMatches(query).forEach(({ node, start, end }) => {
    if (!nodeGroups.has(node)) nodeGroups.set(node, [])
    nodeGroups.get(node).push([start, end])
  })
  nodeGroups.forEach((positions, node) => {
    const text = node.nodeValue
    const parent = node.parentNode
    if (!parent) return
    const fragment = document.createDocumentFragment()
    let cursor = 0
    positions.sort((a, b) => a[0] - b[0]).forEach(([start, end]) => {
      if (start < cursor || end > text.length) return
      if (start > cursor) fragment.appendChild(document.createTextNode(text.slice(cursor, start)))
      const mark = document.createElement('mark')
      mark.className = 'artifact-search-highlight'
      mark.textContent = text.slice(start, end)
      fragment.appendChild(mark)
      cursor = end
    })
    if (cursor < text.length) fragment.appendChild(document.createTextNode(text.slice(cursor)))
    parent.replaceChild(fragment, node)
  })

  state.artifactSearchMatches = [...content.querySelectorAll('mark.artifact-search-highlight')]
  state.artifactSearchIndex = state.artifactSearchMatches.length ? 0 : -1
  if (state.artifactSearchMatches.length) scrollArtifactSearchToMatch(0, { behavior: 'auto' })
  renderArtifactSearchStatus()
}

function renderArtifactSearchStatus() {
  const summary = $('#artifact-search-summary')
  const total = state.artifactSearchMatches.length
  const query = state.artifactSearch.trim()
  if (!summary) return
  if (!query || !artifactInlineSearchAvailable(state.artifact, state.artifactView)) {
    summary.textContent = ''
    $('#artifact-search-prev').disabled = true
    $('#artifact-search-next').disabled = true
    return
  }
  summary.textContent = total
    ? t('{current} / {total}', { current: state.artifactSearchIndex + 1, total })
    : t('No matches found')
  $('#artifact-search-prev').disabled = total === 0
  $('#artifact-search-next').disabled = total === 0
}

function scrollArtifactSearchToMatch(index, { behavior = 'smooth' } = {}) {
  if (!state.artifactSearchMatches.length) return
  const current = ((index % state.artifactSearchMatches.length) + state.artifactSearchMatches.length) % state.artifactSearchMatches.length
  state.artifactSearchIndex = current
  state.artifactSearchMatches.forEach((match, matchIndex) => {
    match.classList.toggle('current', matchIndex === current)
  })
  const target = state.artifactSearchMatches[current]
  if (target?.isConnected) target.scrollIntoView({ behavior, block: 'center', inline: 'nearest' })
  renderArtifactSearchStatus()
}

function navigateArtifactSearch(step) {
  if (!state.artifactSearchMatches.length) return
  const next = state.artifactSearchIndex < 0 ? 0 : state.artifactSearchIndex + step
  scrollArtifactSearchToMatch(next)
}

function formatFileSize(value) {
  const bytes = Number(value) || 0
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

  function jumpArtifactToLine(line, column = 1) {
    if (!state.artifact || !Number.isInteger(Number(line)) || Number(line) < 1) return
    if (state.artifact.kind === 'text') setArtifactView('source')
    requestAnimationFrame(() => {
      const source = $('#artifact-content .artifact-source')
      const node = source?.firstChild
      if (!source || !node) return
      const lines = String(node.textContent || '').split('\n')
      const targetLine = Math.min(lines.length, Math.max(1, Number(line)))
      const start = lines.slice(0, targetLine - 1).reduce((total, value) => total + value.length + 1, 0)
      const offset = Math.min(start + Math.max(0, Number(column || 1) - 1), start + (lines[targetLine - 1]?.length || 0))
      const lineEnd = start + (lines[targetLine - 1]?.length || 0)
      const range = document.createRange()
      range.setStart(node, Math.min(offset, node.length))
      range.setEnd(node, Math.min(Math.max(offset, lineEnd), node.length))
      const selection = window.getSelection()
      selection.removeAllRanges()
      selection.addRange(range)
      const rect = range.getBoundingClientRect()
      $('#artifact-content').scrollBy({ top: rect.top - $('#artifact-content').getBoundingClientRect().top - 90, behavior: 'smooth' })
    })
  }
  function goToPdfPage(page) {
    return richArtifactReader?.goToPage?.(page)
  }

  return {
    bind,
    close: closeArtifactRail,
    goToPdfPage,
    jumpToLine: jumpArtifactToLine,
    open: openArtifact,
    refresh: refreshArtifact,
    render: renderArtifact,
    reopenEpubSource: reopenEpubComment,
    save: saveArtifact,
    setView: setArtifactView,
  }
}
