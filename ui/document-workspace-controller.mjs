import { fuzzyFileLabel, reviewableFileKind } from './composer-tools.mjs'
import { fileDisplayName } from './document-review.mjs'
import { epubCommentSource } from './epub-comment-provider.mjs'
import { documentCommentSource } from './comment-source-providers.mjs'
import { getLocale, t } from './i18n.mjs'
import { createLargeMarkdownHost, createReaderNotesHost } from './large-markdown-host.mjs'

// Candidate resolution only; the WSL transport rechecks canonical file grants.
export function resolveWslDocumentLink(file, target) {
  if (typeof target !== 'string' || new TextEncoder().encode(target).length>8192 || /[\u0000-\u001f\u007f]/u.test(target)) throw Error('Invalid WSL document link')
  target=target.replaceAll('\\','/')
  const base=new URL('file:///')
  if(target.startsWith('/')&&!target.startsWith('//')) {base.pathname=(String(file.documentRoot).replace(/\/$/u,'')+'/').split('/').map(encodeURIComponent).join('/');target=target.replace(/^\/+/u,'')}
  else base.pathname=String(file.path).split('/').map(encodeURIComponent).join('/')
  const url=new URL(target,base)
  if(url.protocol!=='file:' || url.hostname&&url.hostname!=='localhost')throw Error('WSL links require an authorized local file')
  const path=decodeURIComponent(url.pathname)
  if(!path.startsWith('/')||path.includes('\0'))throw Error('Invalid WSL document path')
  return {path,fragment:url.hash}
}

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
    hideSelection,
    setSelection,
    openCommentForSelection,
    openWorkspaceTool,
    openResources,
    renderSessionMap,
    openBrowserUrl,
    readingTypography = () => ({}),
    reportClientError,
  } = view
  const $ = (selector) => document.querySelector(selector)
  const toast = notify
  const showError = reportError
  let artifactSearchTimer = null
  let epubReadingStateTimer = null
  let largeMarkdownFrame = null
  let largeMarkdownFile = null
  let largeMarkdownHost = null
  let readerNotesHost = null

  function releaseLargeMarkdown(file) {
    const lease = file?.largeMarkdown || file?.reader
    if (!lease?.id) return
    gatewayFetch('/studio/markdown/close', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: lease.id }), keepalive: true,
    }).catch(() => {})
  }

  function disposeLargeMarkdown() {
    readerNotesHost?.dispose(); readerNotesHost = null
    if (largeMarkdownFile?.format === 'epub' && largeMarkdownFile.readingState) {
      clearTimeout(epubReadingStateTimer); epubReadingStateTimer = null
      persistEpubReadingState(largeMarkdownFile).catch(error => reportClientError(error))
    }
    largeMarkdownHost?.dispose()
    largeMarkdownHost = null
    readerPresentationSignature = ''
    largeMarkdownFrame?.remove()
    largeMarkdownFrame = null
    releaseLargeMarkdown(largeMarkdownFile)
    largeMarkdownFile = null
  }

  let readerPresentationSignature = ''
  function syncLargeMarkdownTheme() {
    if (!largeMarkdownHost || !largeMarkdownFrame) return
    const style = getComputedStyle(document.documentElement)
    const palette = {}
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = 1
    const context = canvas.getContext('2d', { willReadFrequently: true })
    for (const [key, variable] of Object.entries({panel:'panel',soft:'panel-soft',text:'text',muted:'muted',faint:'faint',border:'border',brand:'brand','brand-soft':'brand-soft'})) {
      const value = style.getPropertyValue('--'+variable).trim()
      if (!context || !value || !CSS.supports('color',value)) continue
      context.clearRect(0,0,1,1);context.fillStyle=value;context.fillRect(0,0,1,1)
      palette[key]='#'+[...context.getImageData(0,0,1,1).data].map(channel=>channel.toString(16).padStart(2,'0')).join('')
    }
    const settings = {theme:document.documentElement.dataset.theme==='dark'?'dark':'light',toolbar:'host',locale:getLocale(),palette}
    for (const [key,variable] of Object.entries({fontFamily:'ui-font-family',contentFontFamily:'content-font-family',codeFontFamily:'code-font-family'})) {
      const value=style.getPropertyValue('--'+variable).trim()
      if(value)settings[key]=value
    }
    for (const [key,variable] of Object.entries({fontSize:'ui-font-size',contentFontSize:'content-font-size',codeFontSize:'code-font-size'})) {
      const value=parseFloat(style.getPropertyValue('--'+variable))
      if(Number.isFinite(value)&&value>=11&&value<=32)settings[key]=value
    }
    const weight=Number(style.getPropertyValue('--content-font-weight'))
    if(Number.isInteger(weight)&&weight>=100&&weight<=900)settings.contentFontWeight=weight
    if (largeMarkdownFile?.format === 'epub') settings.epub = { theme: largeMarkdownFile.readingState?.theme || 'light', typography: readingTypography() }
    const notesRoot = $('#artifact-reader-shell')
    if (notesRoot?.style) for (const [key,value] of Object.entries(palette)) notesRoot.style.setProperty('--reader-'+key,value)
    const signature=JSON.stringify(settings)
    if(signature===readerPresentationSignature)return
    readerPresentationSignature=signature
    const host = largeMarkdownHost
    host.settings(settings).catch(error=>{
      if (largeMarkdownHost !== host) return
      readerPresentationSignature='';showError(error)
    })
  }

  function bind() {
    $('#close-artifact')?.addEventListener('click', closeArtifactRail)
    $('#refresh-artifact')?.addEventListener('click', () => refreshArtifact().catch(showError))
    $('#artifact-preview')?.addEventListener('click', () => setArtifactView('preview'))
    $('#artifact-source')?.addEventListener('click', () => setArtifactView('source'))
    $('#artifact-edit')?.addEventListener('click', () => setArtifactView('edit'))
    $('#artifact-save')?.addEventListener('click', () => saveArtifact().catch(showError))
    $('#artifact-outline-toggle')?.addEventListener('click', toggleArtifactOutline)
    $('#artifact-search-toggle')?.addEventListener('click', toggleArtifactSearch)
    $('#artifact-search-input')?.addEventListener('input', handleArtifactSearchInput)
    $('#artifact-search-input')?.addEventListener('keydown', handleArtifactSearchKeydown)
    $('#artifact-search-prev')?.addEventListener('click', () => navigateArtifactSearch(-1))
    $('#artifact-search-next')?.addEventListener('click', () => navigateArtifactSearch(1))
    document.addEventListener('keydown', handleArtifactNavigationKeydown)
    const themeObserver = new MutationObserver(syncLargeMarkdownTheme)
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] })
    window.addEventListener('beforeunload', event => {
      if (state.artifact?.kind === 'large-markdown' && (largeMarkdownHost?.dirty || largeMarkdownHost?.busy)) { event.preventDefault(); event.returnValue = '' }
    })
  }

async function openReaderLink(file, reference) {
  if (state.artifact !== file) return
  if (/^(https?:|mailto:)/i.test(reference.target)) return openBrowserUrl(reference.target)
  const local=reference.localPath ? {path:reference.localPath,fragment:reference.target.includes('#')?reference.target.slice(reference.target.indexOf('#')):''}
    : file.executionEnvironment==='wsl' ? resolveWslDocumentLink(file,reference.target) : null
  if(local) {
    if(local.path===file.path&&local.fragment&&largeMarkdownHost) return largeMarkdownHost.navigateFragment(local.fragment)
    return openArtifact({root:file.root,path:local.path,readerFragment:local.fragment,
      sourceSessionKey:file.sourceSessionKey},{allowDetachedRoot:true,returnTool:file.returnTool})
  }
  throw Error(t('This link is outside the authorized document directories or uses an unsupported scheme.'))
}

async function openArtifact(file, { allowDetachedRoot = false, returnTool = '', viewState = null, discardUnsaved = false } = {}) {
  if (largeMarkdownHost?.busy) return
  if (largeMarkdownHost?.dirty && state.artifact) state.artifact.dirty = true
  const thread = selectedThread()
  if (!thread?.cwd && !allowDetachedRoot) throw new Error(t('The current session has no project directory, so the file cannot be opened safely.'))
  const root = String(file.root || thread?.cwd || '')
  const sourceSessionKey = String(file.sourceSessionKey || '')
  if (!root) throw new Error(t('The current session has no project directory, so the file cannot be opened safely.'))
  const path = fuzzyFileLabel(file)
  const requestedEpubCfi = String(file.epubCfi || '')
  if (!path) throw new Error(t('The file path is empty.'))
  if (requestedEpubCfi && state.artifact?.format === 'epub' && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    if (file.epubBookHash && file.epubBookHash !== state.artifact.hash) throw Error(t('The book has changed since this reference was captured'))
    if (largeMarkdownHost && state.artifact.format === 'epub') {
      if (file.readerReference) await largeMarkdownHost.navigateReference(file.readerReference)
      else await largeMarkdownHost.navigateLocation({kind:'epub',cfi:requestedEpubCfi})
    }
    else {
      state.artifact.readingState = { ...(state.artifact.readingState || {}), cfi: requestedEpubCfi }
      renderArtifact()
    }
    return
  }
  if (!discardUnsaved && state.artifact?.dirty && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    return
  }
  if (!discardUnsaved && state.artifact?.dirty && (state.artifact.root !== root || state.artifact.path !== path)) {
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
  disposeLargeMarkdown()
  const requestId = randomId()
  state.artifact = { root, path, kind, readerReference: file.readerReference, readerFragment: file.readerFragment, requestId, threadKey: selectedStateKey(), returnTool, sourceSessionKey, loading: true }
  try {
    const response = await gatewayFetch('/studio/reader/open', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ root, path }),
    })
    if (!response.ok) {
      const result = await response.json().catch(() => null)
      const message = result?.error?.message || `HTTP ${response.status}`
      throw new Error(message)
    }
    const result = await response.json()
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) {
      releaseLargeMarkdown(result)
      return
    }
    if (!result.reader?.id || typeof result.reader.url !== 'string') throw Error(t('Invalid reader response'))
    let readingState
    if (result.format === 'epub') {
      try {
        if (!result.bookHash) throw Error(t('Invalid EPUB response'))
        if (file.epubBookHash && file.epubBookHash !== result.bookHash) throw Error(t('The book has changed since this reference was captured'))
        readingState = await loadEpubReadingState(root, path, result.bookHash)
        if (requestedEpubCfi) readingState.cfi = requestedEpubCfi
      } catch (error) { releaseLargeMarkdown(result); throw error }
      if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) { releaseLargeMarkdown(result); return }
    }
    state.artifact = { ...result, readingState, hash: result.bookHash, largeMarkdown: result.reader, readerSourceAvailable: result.sourceAvailable,
      readerReference: file.readerReference, readerFragment: file.readerFragment, kind: 'large-markdown', requestId, threadKey: selectedStateKey(), returnTool, sourceSessionKey, loading: false }
    largeMarkdownFile = state.artifact
    state.artifactView = 'preview'
    if (viewState) {
      if (['preview', 'source', 'edit'].includes(viewState.view)) state.artifactView = state.artifact.readerSourceAvailable === false ? 'preview' : state.artifact.readOnly && viewState.view === 'edit' ? 'source' : viewState.view
      if (state.artifact.format === 'pdf') {
        state.artifact.page = viewState.page || 1
        state.artifact.pdfPresentation = viewState.pdfPresentation
      }
      state.artifactOutlineOpen = Boolean(viewState.outlineOpen)
      state.artifactOutlineFilter = viewState.outlineFilter || ''
      state.artifactOutlineCollapsed = new Set(viewState.outlineCollapsed || [])
      state.artifactSearch = viewState.search || ''
      state.artifactSearchOpen = Boolean(viewState.searchOpen)
    }
    await renderArtifact()
    if (viewState) requestAnimationFrame(() => {
      if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
      $('#artifact-content').scrollTop = viewState.scrollTop || 0
      $('#artifact-content').scrollLeft = viewState.scrollLeft || 0
    })
  } catch (error) {
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = { ...state.artifact, loading: false, error: error?.message || String(error) }
    renderArtifact()
    throw error
  }
}

async function refreshArtifact() {
  if (!state.artifact) return
  if (largeMarkdownHost?.busy) return
  if (largeMarkdownHost?.dirty) state.artifact.dirty = true
  if (state.artifact.dirty && !confirm(t('Reloading will discard unsaved changes. Continue?'))) return
  await openArtifact({ root: state.artifact.root, path: state.artifact.path, sourceSessionKey: state.artifact.sourceSessionKey }, { returnTool: state.artifact.returnTool, discardUnsaved: true })
}

function closeArtifactRail({ restoreMap = true, restoreWorkspace = true } = {}) {
  if (largeMarkdownHost?.busy) return false
  if (largeMarkdownHost?.dirty && state.artifact) state.artifact.dirty = true
  const sourceSessionKey = state.artifact?.sourceSessionKey || ''
  if (state.artifact?.dirty && !confirm(t('The current document has unsaved changes. Close it anyway?'))) return false
  const returnTool = restoreWorkspace ? state.artifact?.returnTool : ''
  const artifactThreadKey = state.artifact?.threadKey
  $('#artifact-rail').classList.add('hidden')
  disposeLargeMarkdown()
  resetArtifactOutline()
  state.artifact = null
  resetArtifactSearch()
  hideSelection()
  if (returnTool && artifactThreadKey === selectedStateKey()) {
    if (returnTool === 'resources') {
      openResources()
      return
    }
    Promise.resolve(openWorkspaceTool(returnTool, sourceSessionKey)).catch(showError)
    return
  }
  if (restoreMap && $('#annotation-rail').classList.contains('hidden') && $('#favorites-rail').classList.contains('hidden')) renderSessionMap()
  return true
}

function setArtifactView(view) {
  const file = state.artifact
  if (!file || file.kind !== 'large-markdown' || !['preview','source','edit'].includes(view)) return
  if (view !== 'preview' && file.readerSourceAvailable === false || view === 'edit' && file.readOnly) return
  return largeMarkdownHost?.setView(view).catch(showError)
}

function toggleArtifactSearch() {
  if ($('#artifact-search-toggle').classList.contains('hidden')) return
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
  largeMarkdownHost?.setOutline(state.artifactOutlineOpen).catch(showError)
  $('#artifact-outline-toggle').classList.toggle('active', state.artifactOutlineOpen)
  $('#artifact-outline-toggle').setAttribute('aria-expanded', String(state.artifactOutlineOpen))
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
  state.artifactOutlineFilter = ''
  state.artifactOutlineActiveId = ''
  state.artifactOutlineCollapsed = new Set()
  state.artifactOutlineOpen = false
  $('#artifact-outline-toggle')?.classList.add('hidden')
}

async function saveArtifact() {
  if (state.artifact?.kind === 'large-markdown' && !state.artifact.readOnly && await largeMarkdownHost?.save()) toast('File saved')
}

function renderArtifact() {
  const rail = $('#artifact-rail')
  const file = state.artifact
  if (!file) {
    rail.classList.add('hidden')
    resetArtifactSearch()
    return
  }
  if (state.activeRightWorkspace !== 'document') {
    rail.classList.add('hidden')
    return
  }
  applyRightRailWidth()
  rail.classList.remove('hidden')
  const artifactTitle = fileDisplayName(file.path)
  const artifactPath = file.relativePath || file.path
  $('#artifact-title').textContent = artifactTitle
  $('#artifact-title').title = artifactTitle
  $('#artifact-path').textContent = artifactPath
  $('#artifact-path').title = artifactPath
  const closeButton = $('#close-artifact')
  const returnLabel = file.returnTool === 'files' ? t('Back to Files') : file.returnTool === 'review' ? t('Back to Git Review') : file.returnTool === 'resources' ? t('Back to Resources') : t('Close document')
  closeButton.classList.toggle('returning', Boolean(file.returnTool))
  closeButton.title = returnLabel
  closeButton.setAttribute('aria-label', returnLabel)
  $('#artifact-loading').classList.toggle('hidden', !file.loading)
  $('#artifact-error').classList.toggle('hidden', !file.error)
  $('#artifact-error-message').textContent = file.error || ''
  const largeMarkdownReady = !file.loading && !file.error && file.kind === 'large-markdown' && Boolean(file.largeMarkdown?.url)
  const ready = largeMarkdownReady
  const content = $('#artifact-content')
  $('#artifact-reader-shell').classList.toggle('hidden', !ready)
  content.classList.toggle('hidden', !ready)
  const editable = largeMarkdownReady && !file.readOnly
  $('#artifact-title').textContent = `${fileDisplayName(file.path)}${file.dirty ? ' •' : ''}`
  $('#artifact-view-switch').classList.toggle('hidden', !largeMarkdownReady)
  $('#artifact-preview').disabled = !file.readerReady || Boolean(file.saving)
  $('#artifact-source').classList.toggle('hidden', largeMarkdownReady && file.readerSourceAvailable === false)
  $('#artifact-source').disabled = largeMarkdownReady && (!file.readerReady || Boolean(file.saving))
  $('#artifact-edit').disabled = largeMarkdownReady && (!file.readerReady || Boolean(file.saving))
  $('#close-artifact').disabled = largeMarkdownReady && Boolean(file.saving)
  $('#refresh-artifact').disabled = largeMarkdownReady && Boolean(file.saving)
  $('#artifact-preview').classList.toggle('active', state.artifactView === 'preview')
  $('#artifact-source').classList.toggle('active', state.artifactView === 'source')
  $('#artifact-edit').classList.toggle('active', state.artifactView === 'edit')
  $('#artifact-edit').classList.toggle('hidden', !editable)
  $('#artifact-save').classList.toggle('hidden', state.artifactView !== 'edit')
  $('#artifact-save').disabled = !file.dirty || Boolean(file.saving)
  const canSearch = largeMarkdownReady && state.artifactView !== 'edit'
  const canInlineSearch = canSearch
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
    $('#artifact-outline-toggle').classList.add('hidden')
    return
  }
  if (largeMarkdownReady) {
    $('#artifact-outline-toggle').classList.toggle('hidden', !file.readerHasOutline)
    $('#artifact-outline-toggle').classList.toggle('active', state.artifactOutlineOpen)
    $('#artifact-outline-toggle').setAttribute('aria-expanded', String(state.artifactOutlineOpen))
    $('#artifact-search-toggle').disabled = !file.readerReady
    $('#artifact-outline-toggle').disabled = !file.readerHasOutline || Boolean(file.saving)
    $('#artifact-meta').textContent = `${file.lineCount == null ? formatFileSize(file.size) : t('{lines} lines · {size}', { lines: file.lineCount, size: formatFileSize(file.size) })}${file.readOnly ? ` · ${t('Read only')}` : ''}`
    $('#artifact-hint').textContent = file.readerStatus || ''
    content.className = 'artifact-content artifact-large-markdown'
    if (!largeMarkdownFrame || largeMarkdownFile !== file) {
      largeMarkdownFrame?.remove()
      largeMarkdownFile = file
      largeMarkdownFrame = document.createElement('iframe')
      largeMarkdownFrame.className = 'artifact-large-markdown-frame'
      largeMarkdownFrame.title = fileDisplayName(file.path)
      if ((file.readerReference || file.readerFragment || file.format === 'epub' && file.readingState?.cfi) && largeMarkdownFrame.style) largeMarkdownFrame.style.visibility = 'hidden'
      largeMarkdownFrame.setAttribute('sandbox', 'allow-scripts allow-same-origin allow-forms')
      largeMarkdownFrame.setAttribute('referrerpolicy', 'no-referrer')
      largeMarkdownFrame.src = file.largeMarkdown.url
      largeMarkdownFrame.addEventListener('load', () => {
        syncLargeMarkdownTheme()
        if (state.artifact === file && file.readerLine) largeMarkdownHost?.navigateLine(file.readerLine).catch(showError)
      })
      content.replaceChildren(largeMarkdownFrame)
      const commentSelection = selection => ({ quote: selection.quote, itemId: null, turnId: null,
        source: selection.location?.kind === 'epub' ? epubCommentSource({ root: file.root, filePath: file.path,
          bookHash: file.hash, cfiRange: selection.location.cfi, href: selection.location.href,
          chapterLabel: file.readingState?.chapterLabel, readerReference: selection })
          : documentCommentSource({ kind: 'fileRange', root: file.root, filePath: file.path,
          baseHash: selection.revision, startLine: selection.startLine, endLine: selection.endLine,
          readerReference: selection }) })
      readerNotesHost = createReaderNotesHost($('#artifact-reader-shell'), $('#artifact-notes-tools'), {
        translate: t, onError: showError,
        onAction: openCommentForSelection ? selection => openCommentForSelection(commentSelection(selection)) : undefined,
      })
      largeMarkdownHost = createLargeMarkdownHost(largeMarkdownFrame, snapshot => {
        if (state.artifact !== file) return
        void readerNotesHost?.update(snapshot, largeMarkdownHost).catch(showError)
        if (snapshot.initialized && largeMarkdownFrame?.style) largeMarkdownFrame.style.visibility = ''
        if (file.format === 'epub' && snapshot.initialized && snapshot.formatState?.ready) {
          const next = snapshot.formatState
          const reading = { cfi: next.cfi, chapterLabel: next.chapterLabel, progress: next.progress,
            fontScale: next.fontScale, theme: next.theme, flow: next.flow, tocOpen: next.tocOpen }
          if (JSON.stringify(file.readingState) !== JSON.stringify(reading)) {
            file.readingState = reading
            scheduleEpubReadingState(file)
          }
        }
        if (file.format === 'pdf' && snapshot.formatState?.ready) {
          file.page = snapshot.formatState.page
          file.pdfPresentation = { zoom: snapshot.formatState.zoom, continuous: snapshot.formatState.continuous }
        }
        file.readerSourceAvailable = snapshot.capabilities?.source
        file.readerReady = snapshot.ready
        file.readerReadingReady = snapshot.readingReady
        file.readerHasOutline = snapshot.hasOutline
        file.dirty = Boolean(snapshot.dirty)
        file.saving = Boolean(snapshot.busy)
        file.readerStatus = snapshot.status
        if (snapshot.size != null) file.size = snapshot.size
        if (snapshot.lineCount != null) file.lineCount = snapshot.lineCount
        if (snapshot.view) state.artifactView = snapshot.view
        if (snapshot.outlineOpen != null) state.artifactOutlineOpen = snapshot.outlineOpen
        if (snapshot.error) file.error = snapshot.error
        renderArtifact()
      }, selection => {
        if (state.artifact !== file) return
        if (!readerNotesHost?.capture(selection)) setSelection(commentSelection(selection), selection.rect)
      }, { initialView: state.artifactView, onOpenRequested: () => openResources(),
        onError: showError, onLinkRequested: reference => openReaderLink(file, reference),
        initialReference: file.readerReference, initialFragment: file.readerFragment,
        presentation: file.format === 'epub' ? { epub: { theme: file.readingState?.theme || 'light',
          flow: file.readingState?.flow || 'paginated', fontScale: file.readingState?.fontScale || 1,
          typography: readingTypography() } } : file.format === 'pdf' && file.pdfPresentation ? { pdf: file.pdfPresentation } : undefined,
        initialLocation: file.readerReference ? undefined : file.format === 'epub' && file.readingState?.cfi
          ? {kind:'epub',cfi:file.readingState.cfi}
          : file.format === 'pdf' && file.page ? {kind:'pdf',pages:[{page:file.page,rects:[{x:0,y:0,width:1,height:0.000001}]}]} : undefined,
      })
      delete file.readerReference
      delete file.readerFragment
    }
    syncLargeMarkdownTheme()
    renderArtifactSearchStatus()
    return
  }
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

async function reopenEpubComment(anchor) {
  await openArtifact({
    root: anchor.root || selectedThread()?.cwd,
    path: anchor.filePath,
    epubCfi: anchor.cfiRange,
    epubBookHash: anchor.bookHash,
    readerReference: anchor.readerReference,
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
  renderArtifactSearchStatus()
  clearTimeout(artifactSearchTimer)
}

function handleArtifactSearchInput(event) {
  state.artifactSearch = event.target.value
  clearTimeout(artifactSearchTimer)
  if (state.artifact?.kind === 'large-markdown') {
    const file = state.artifact
    const query = state.artifactSearch
    state.artifactSearchMatches = []
    state.artifactSearchIndex = -1
    renderArtifactSearchStatus()
    artifactSearchTimer = setTimeout(async () => {
      try {
        const matches = await largeMarkdownHost?.search(query) || []
        if (state.artifact !== file || state.artifactSearch !== query) return
        state.artifactSearchMatches = matches
        state.artifactSearchIndex = matches.length ? 0 : -1
        if (matches.length) scrollArtifactSearchToMatch(0)
        else renderArtifactSearchStatus()
      } catch (error) { if (error?.name !== 'AbortError' && state.artifact === file) showError(error) }
    }, 140)
    return
  }
}

function handleArtifactSearchKeydown(event) {
  if (event.key !== 'Enter') return
  event.preventDefault()
  if (!state.artifactSearch.trim() || state.artifactSearchMatches.length === 0) return
  navigateArtifactSearch(event.shiftKey ? -1 : 1)
}

function renderArtifactSearchStatus() {
  const summary = $('#artifact-search-summary')
  const total = state.artifactSearchMatches.length
  const query = state.artifactSearch.trim()
  if (!summary) return
  if (!query || state.artifact?.kind !== 'large-markdown') {
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

function scrollArtifactSearchToMatch(index) {
  if (!state.artifactSearchMatches.length) return
  const current = ((index % state.artifactSearchMatches.length) + state.artifactSearchMatches.length) % state.artifactSearchMatches.length
  state.artifactSearchIndex = current
  Promise.resolve(largeMarkdownHost?.navigateMatch(state.artifactSearchMatches[current])).catch(showError)
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

  async function jumpArtifactToLine(line) {
    if (!state.artifact || !Number.isSafeInteger(Number(line)) || Number(line) < 1) return
    const file = state.artifact
    file.readerLine = Number(line)
    const host = largeMarkdownHost
    if (host) {
      await host.ready
      if (state.artifact !== file || largeMarkdownHost !== host) throw Error('Document changed before navigation')
      await host.navigateLine(Number(line))
    }
  }

  async function goToPdfPage(page) {
    if (state.artifact?.format !== 'pdf' || !largeMarkdownHost) return
    await largeMarkdownHost.ready
    return largeMarkdownHost.navigateLocation({kind:'pdf',pages:[{page,rects:[{x:0,y:0,width:1,height:0.000001}]}]})
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
