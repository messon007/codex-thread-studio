import { backendDescriptor } from './backends.mjs'
import {
  commentSelectionSnapshot,
  createCommentDraft,
} from './comment-core.mjs'
import {
  chatCommentSource,
  documentCommentSource,
  relocateDocumentComment,
} from './comment-source-providers.mjs'
import { createFileRangeTarget } from './document-review.mjs'
import {
  autoFavoriteTitle,
  favoriteCopyText,
  favoriteSourceKey,
  normalizeFavoriteTags,
  questionForTurn,
} from './favorites.mjs'
import {
  formatDate as formatLocalizedDate,
  getLocale,
  t,
} from './i18n.mjs'
import { sessionMapVisibleText } from './session-map.mjs'

export function createReviewNotesState() {
  return {
    annotationDrafts: {},
    annotationAdditional: {},
    pendingSelection: null,
    pendingAnnotation: null,
    favorites: [],
    favoriteIndex: [],
    favoriteTotal: 0,
    favoriteQuery: '',
    favoriteScope: 'global',
    pendingFavorite: null,
    selectedFavorite: null,
    favoriteEditMode: false,
  }
}

export function createReviewNotesController({
  state,
  commentSources,
  gatewayFetch,
  view,
  randomId,
  persistPreferences,
  notify = () => {},
  reportError = console.error,
}) {
  const {
    selectedStateKey,
    selectedThread,
    threadTitle,
    closeActionMenus,
    activateRightWorkspace,
    syncRightWorkspaceLaunchers,
    renderArtifact,
    renderSessionMap,
    openArtifact,
    setArtifactView,
    reopenEpubSource,
    goToPdfPage,
    openBrowserUrl,
    renderMarkdown,
    renderedItem,
    pauseTranscript,
    switchBackend,
    waitForBackend,
    loadThreads,
    selectThread,
  } = view
  const $ = (selector) => document.querySelector(selector)
  const $$ = (selector) => [...document.querySelectorAll(selector)]
  const toast = notify
  const showError = reportError
  const waitFor = waitForBackend
  const transcriptScrollFollower = { pause: pauseTranscript }
  const richArtifactReader = { goToPage: goToPdfPage }
  const reopenEpubComment = reopenEpubSource
  let annotationPersistTimer = null
  let favoritesSearchTimer = null

  function bind() {
    $('#composer-review-open')?.addEventListener('click', openAnnotationRail)
    $('#composer-review-insert')?.addEventListener('click', insertAnnotations)
    $('#transcript')?.addEventListener('mouseup', captureTranscriptSelection)
    $('#artifact-content')?.addEventListener('mouseup', captureArtifactSelection)
    $('#open-thread-comments')?.addEventListener('click', openAnnotationRail)
    $('#open-thread-favorites')?.addEventListener('click', () => openFavoritesRail('session'))
    $('#selection-popover')?.addEventListener('mousedown', (event) => event.preventDefault())
    $('#selection-comment')?.addEventListener('click', openAnnotationFromSelection)
    $('#selection-favorite')?.addEventListener('click', openFavoriteFromSelection)
    $('#close-annotation-rail')?.addEventListener('click', closeAnnotationRail)
    $('#annotation-form')?.addEventListener('submit', addAnnotation)
    $('#close-annotation-dialog')?.addEventListener('click', closeAnnotationDialog)
    $('#cancel-annotation')?.addEventListener('click', closeAnnotationDialog)
    $('#clear-annotations')?.addEventListener('click', clearAnnotations)
    $('#insert-annotations')?.addEventListener('click', insertAnnotations)
    $('#annotation-additional')?.addEventListener('input', saveAnnotationAdditional)
    $('#open-favorites')?.addEventListener('click', () => {
      closeActionMenus()
      openFavoritesRail('global')
    })
    $('#export-favorites')?.addEventListener('click', () => exportFavorites().catch(showError))
    $('#close-favorites')?.addEventListener('click', closeFavoritesRail)
    $('#favorites-search')?.addEventListener('input', handleFavoritesSearch)
    $('#favorites-list')?.addEventListener('click', handleFavoriteListClick)
    $('#favorite-form')?.addEventListener('submit', saveFavorite)
    $('#close-favorite-dialog')?.addEventListener('click', closeFavoriteDialog)
    $('#cancel-favorite')?.addEventListener('click', closeFavoriteDialog)
    $('#favorite-include-question')?.addEventListener('change', renderFavoriteQuestionOption)
    $('#close-favorite-detail')?.addEventListener('click', closeFavoriteDetail)
    $('#copy-favorite')?.addEventListener('click', () => copySelectedFavorite().catch(showError))
    $('#edit-favorite')?.addEventListener('click', editSelectedFavorite)
    $('#delete-favorite')?.addEventListener('click', () => deleteSelectedFavorite().catch(showError))
    $('#open-favorite-source')?.addEventListener('click', () => openSelectedFavoriteSource().catch(showError))
  }

  function setSelection(selection, rect = null, { allowFavorite = false } = {}) {
    state.pendingSelection = selection
    if (rect) positionSelectionPopover(rect, { allowFavorite })
  }

  function openCommentForSelection(selection) {
    state.pendingSelection = selection
    openAnnotationFromSelection()
  }

  function openFavoriteForResource(resource, occurrence) {
    const thread = selectedThread()
    if (!thread || !occurrence?.turnId || !occurrence?.itemId) {
      toast(t('Unable to locate the resource in its message'), 'error')
      return
    }
    const turn = state.model.turns.find((candidate) => String(candidate.id) === String(occurrence.turnId))
    const target = resource.target?.url || resource.target?.path || resource.raw
    const location = resource.target?.line
      ? `${target}:${resource.target.line}${resource.target.column ? `:${resource.target.column}` : ''}`
      : target
    const content = resource.target?.url ? String(location) : `\`${String(location).replaceAll('\`', '')}\``
    state.favoriteEditMode = false
    state.pendingFavorite = {
      id: randomId(),
      scope: 'selection',
      backend: state.backend,
      threadId: state.selectedId,
      threadTitle: threadTitle(thread),
      projectPath: thread.cwd || '',
      turnId: String(occurrence.turnId),
      itemId: String(occurrence.itemId),
      title: String(resource.display || autoFavoriteTitle(location)),
      presentation: 'resource',
      question: turn ? questionForTurn(turn) : '',
      content,
      note: '',
      tags: [],
      createdAt: new Date().toISOString(),
    }
    populateFavoriteDialog(state.pendingFavorite)
  }

function captureTranscriptSelection() {
  const selection = window.getSelection()
  const text = selection?.toString().trim()
  if (!text || selection.rangeCount === 0) return hideSelectionPopover()
  const range = selection.getRangeAt(0)
  const transcript = $('#transcript')
  if (!transcript.contains(range.commonAncestorContainer)) return hideSelectionPopover()
  const element = range.commonAncestorContainer.nodeType === Node.ELEMENT_NODE
    ? range.commonAncestorContainer
    : range.commonAncestorContainer.parentElement
  const item = element?.closest('[data-item-id]')
  const turn = element?.closest('[data-turn-id]')
  state.pendingSelection = {
    quote: text.slice(0, 16000),
    itemId: item?.dataset.itemId || null,
    turnId: item?.dataset.turnId || turn?.dataset.turnId || null,
    source: chatCommentSource({
      itemId: item?.dataset.itemId || null,
      turnId: item?.dataset.turnId || turn?.dataset.turnId || null,
    }),
  }
  positionSelectionPopover(range, { allowFavorite: true })
}

function captureArtifactSelection() {
  const selection = window.getSelection()
  const text = selection?.toString().trim()
  if (state.artifact?.kind === 'image') return hideSelectionPopover()
  if (!state.artifact || !text || selection.rangeCount === 0) return hideSelectionPopover()
  const range = selection.getRangeAt(0)
  const content = $('#artifact-content')
  if (!content.contains(range.commonAncestorContainer)) return hideSelectionPopover()
  let hintOffset = 0
  if (state.artifactView === 'source') {
    const source = content.querySelector('.artifact-source')
    if (source) {
      const prefix = document.createRange()
      prefix.selectNodeContents(source)
      prefix.setEnd(range.startContainer, range.startOffset)
      hintOffset = prefix.toString().length
    }
  }
  state.pendingSelection = {
    quote: text.slice(0, 16000),
    itemId: null,
    turnId: null,
    source: documentCommentSource(createFileRangeTarget(state.artifact, text, hintOffset)),
  }
  positionSelectionPopover(range, { allowFavorite: false })
}

function positionSelectionPopover(range, { allowFavorite }) {
  const rect = typeof range?.getBoundingClientRect === 'function' ? range.getBoundingClientRect() : range
  if (!rect) return hideSelectionPopover()
  const popover = $('#selection-popover')
  popover.style.left = `${Math.min(window.innerWidth - 150, Math.max(8, rect.left + rect.width / 2 - 55))}px`
  popover.style.top = `${Math.max(8, rect.top - 39)}px`
  $('#selection-favorite').classList.toggle('hidden', !allowFavorite)
  popover.classList.remove('hidden')
}

function openAnnotationFromSelection() {
  if (!state.pendingSelection?.quote) {
    captureTranscriptSelection()
    if (!state.pendingSelection?.quote) return toast('Select text in the Codex output first', 'error')
  }
  state.pendingAnnotation = commentSelectionSnapshot(state.pendingSelection, commentSources)
  if (!state.pendingAnnotation) return toast('Select the text to comment on again', 'error')
  $('#annotation-quote').textContent = state.pendingAnnotation.excerpt
  $('#annotation-source-hint').textContent = commentSources.describe(state.pendingAnnotation, commentProviderContext(0))
  $('#annotation-comment').value = ''
  $('#annotation-comment').placeholder = state.pendingAnnotation.source?.provider === 'epub'
    ? t('For example: explain the core meaning, context, and key concepts in this passage.')
    : t('Describe the issue and expected change, or add the selection directly to the draft.')
  $('#annotation-error').classList.add('hidden')
  hideSelectionPopover(false)
  $('#annotation-dialog').showModal()
  setTimeout(() => $('#annotation-comment').focus(), 30)
}

function openFavoriteFromSelection() {
  if (!state.pendingSelection?.quote) {
    captureTranscriptSelection()
    if (!state.pendingSelection?.quote) return toast('Select text in the AI output first', 'error')
  }
  const thread = selectedThread()
  const turn = state.model.turns.find((candidate) => String(candidate.id) === String(state.pendingSelection.turnId))
  if (!thread || !state.pendingSelection.turnId || !state.pendingSelection.itemId) {
    return toast('The selected text could not be anchored. Select within a single response.', 'error')
  }
  state.favoriteEditMode = false
  state.pendingFavorite = {
    id: randomId(),
    scope: 'selection',
    backend: state.backend,
    threadId: state.selectedId,
    threadTitle: threadTitle(thread),
    projectPath: thread.cwd || '',
    turnId: String(state.pendingSelection.turnId),
    itemId: String(state.pendingSelection.itemId),
    title: autoFavoriteTitle(state.pendingSelection.quote),
    question: turn ? questionForTurn(turn) : '',
    content: state.pendingSelection.quote,
    note: '',
    tags: [],
    createdAt: new Date().toISOString(),
  }
  hideSelectionPopover(false)
  populateFavoriteDialog(state.pendingFavorite)
}

function hideSelectionPopover(clear = true) {
  $('#selection-popover').classList.add('hidden')
  if (clear) state.pendingSelection = null
}

function closeAnnotationDialog() {
  $('#annotation-dialog').close()
  state.pendingAnnotation = null
  state.pendingSelection = null
  window.getSelection()?.removeAllRanges()
}

function currentAnnotations() {
  return state.selectedId ? state.annotationDrafts[selectedStateKey()] || [] : []
}

function addAnnotation(event) {
  event.preventDefault()
  const comment = $('#annotation-comment').value.trim()
  const errorBox = $('#annotation-error')
  const annotation = state.pendingAnnotation
  if (!state.selectedId || !annotation?.excerpt) {
    errorBox.textContent = 'The selected content is required.'
    errorBox.classList.remove('hidden')
    return
  }
  const drafts = currentAnnotations()
  if (drafts.length >= 32) {
    errorBox.textContent = 'A session can keep up to 32 comments.'
    errorBox.classList.remove('hidden')
    return
  }
  const draft = createCommentDraft({ ...annotation, note: comment }, { registry: commentSources })
  state.annotationDrafts[selectedStateKey()] = [...drafts, draft]
  persistPreferences()
  closeAnnotationDialog()
  renderAnnotationRail()
  renderComposerReviewContext()
  toast('Comment added to reply draft')
}

function openAnnotationRail() {
  activateRightWorkspace('comments')
  renderAnnotationRail()
}
function closeAnnotationRail() {
  $('#annotation-rail').classList.add('hidden')
  if (state.activeRightWorkspace === 'comments') state.activeRightWorkspace = null
  syncRightWorkspaceLaunchers()
  if ($('#favorites-rail').classList.contains('hidden')) {
    if (state.artifact) renderArtifact()
    else renderSessionMap()
  }
}

function renderAnnotationRail() {
  const drafts = currentAnnotations()
  $('#annotation-count').textContent = drafts.length
  setWorkspaceToolCount($('#thread-comments-count'), drafts.length)
  const commentsButton = $('#open-thread-comments')
  const commentsLabel = drafts.length ? `${t('Comments')} · ${drafts.length}` : t('Comments')
  commentsButton.title = commentsLabel
  commentsButton.setAttribute('aria-label', commentsLabel)
  $('#annotation-empty').classList.toggle('hidden', drafts.length > 0)
  $('#annotation-list').classList.toggle('hidden', drafts.length === 0)
  $('#clear-annotations').disabled = !drafts.length && !state.annotationAdditional[selectedStateKey()]
  $('#insert-annotations').disabled = !drafts.length
  $('#annotation-additional').value = state.selectedId ? state.annotationAdditional[selectedStateKey()] || '' : ''
  $('#annotation-list').innerHTML = drafts.map((draft, index) => `<article class="annotation-card" data-draft-id="${escapeHtml(draft.id)}">
    <header><button class="annotation-source" type="button">${escapeHtml(annotationSourceLabel(draft, index))}</button><button class="annotation-delete" type="button" aria-label="${t('Delete comment {index}', { index: index + 1 })}">×</button></header>
    <blockquote>${escapeHtml(draft.excerpt)}</blockquote>${draft.note ? `<p>${escapeHtml(draft.note)}</p>` : ''}
  </article>`).join('')
  $$('.annotation-delete').forEach((button) => button.addEventListener('click', () => deleteAnnotation(button.closest('.annotation-card').dataset.draftId)))
  $$('.annotation-source').forEach((button) => button.addEventListener('click', () => reopenAnnotationSource(button.closest('.annotation-card').dataset.draftId).catch(showError)))
  renderComposerReviewContext()
}

function annotationSourceLabel(draft, index) {
  return commentSources.describe(draft, commentProviderContext(index))
}

async function reopenAnnotationSource(id) {
  const draft = currentAnnotations().find((candidate) => candidate.id === id)
  if (!draft) return
  await commentSources.reopen(draft, commentProviderContext())
}

async function reopenDocumentComment(target, excerpt) {
  await openArtifact({ root: target.root || selectedThread()?.cwd, path: target.filePath })
  setArtifactView('source')
  const source = $('#artifact-content .artifact-source')
  if (!source) return
  const { startOffset, endOffset } = relocateDocumentComment({ anchor: target }, state.artifact, excerpt)
  if (startOffset == null || endOffset == null || endOffset <= startOffset) return
  const node = source.firstChild
  if (!node) return
  const range = document.createRange()
  range.setStart(node, Math.min(startOffset, node.length))
  range.setEnd(node, Math.min(endOffset, node.length))
  const selection = window.getSelection()
  selection.removeAllRanges()
  selection.addRange(range)
  const rect = range.getBoundingClientRect()
  $('#artifact-content').scrollBy({ top: rect.top - $('#artifact-content').getBoundingClientRect().top - 90, behavior: 'smooth' })
}

function renderComposerReviewContext() {
  const drafts = currentAnnotations()
  const context = $('#composer-review-context')
  context.classList.toggle('hidden', !drafts.length)
  if (!drafts.length) return
  $('#composer-review-count').textContent = t('{count} comments ready to send', { count: drafts.length })
  $('#composer-review-source').textContent = t('Waiting to be added')
}

function deleteAnnotation(id) {
  if (!state.selectedId) return
  const key = selectedStateKey()
  state.annotationDrafts[key] = currentAnnotations().filter((draft) => draft.id !== id)
  if (!state.annotationDrafts[key].length) delete state.annotationDrafts[key]
  persistPreferences()
  renderAnnotationRail()
}

function clearAnnotations() {
  if (!state.selectedId || !confirm(t('Clear all comment drafts for this session?'))) return
  delete state.annotationDrafts[selectedStateKey()]
  delete state.annotationAdditional[selectedStateKey()]
  persistPreferences()
  renderAnnotationRail()
}

function saveAnnotationAdditional(event) {
  if (!state.selectedId) return
  const value = event.target.value.slice(0, 32000)
  if (value) state.annotationAdditional[selectedStateKey()] = value
  else delete state.annotationAdditional[selectedStateKey()]
  clearTimeout(annotationPersistTimer)
  annotationPersistTimer = setTimeout(persistPreferences, 300)
}

function buildAnnotationPrompt(drafts, additional = '') {
  const annotations = drafts.map((draft, index) => {
    const anchor = commentSources.promptAnchor(draft, commentProviderContext(index))
    const quote = draft.excerpt.split('\n').map((line) => `> ${line}`).join('\n')
    return t(anchor
      ? 'Comment {index} ({anchor})\nQuote:\n{quote}\n\nMy comment:\n{comment}'
      : 'Comment {index}\nQuote:\n{quote}\n\nMy comment:\n{comment}', {
      index: index + 1,
      anchor,
      quote,
      comment: draft.note || t('No additional comment'),
    })
  }).join('\n\n---\n\n')
  const additionalBlock = additional.trim() ? t('Overall note:\n{text}', { text: additional.trim() }) : ''
  return [...commentSources.promptInstructions(drafts, commentProviderContext()), state.annotationPromptTemplate
    .replaceAll('{{annotations}}', annotations)
    .replaceAll('{{additional}}', additionalBlock)
    .replace(/\n{3,}/g, '\n\n')
    .trim()].filter(Boolean).join('\n\n')
}

function commentProviderContext(index = 0) {
  return {
    index,
    translate: t,
    unknownLabel: t('Saved comment'),
    contentForSource: (source) => state.artifact?.path === source?.anchor?.filePath ? state.artifact.content : null,
    openDocument: reopenDocumentComment,
    openEpubSource: reopenEpubComment,
    openPdfSource: reopenPdfComment,
    openTableSource: reopenTableComment,
    openWebSource: openBrowserUrl,
  }
}

async function reopenPdfComment(anchor) {
  await openArtifact({ root: anchor.root || selectedThread()?.cwd, path: anchor.filePath })
  state.artifact.page = anchor.page
  await richArtifactReader?.goToPage?.(anchor.page)
}

async function reopenTableComment(anchor) {
  await openArtifact({ root: anchor.root || selectedThread()?.cwd, path: anchor.filePath })
}

function insertAnnotations() {
  const drafts = currentAnnotations()
  if (!drafts.length) return
  const prompt = buildAnnotationPrompt(drafts, state.annotationAdditional[selectedStateKey()] || '')
  const composer = $('#composer-input')
  composer.value = [composer.value.trim(), prompt].filter(Boolean).join('\n\n')
  closeAnnotationRail()
  composer.focus()
  renderComposerReviewContext()
  toast('Comment draft inserted into the composer')
}

async function favoriteRequest(path, options = {}) {
  const response = await gatewayFetch(path, {
    cache: 'no-store',
    ...options,
    headers: options.body ? { 'Content-Type': 'application/json', ...(options.headers || {}) } : options.headers,
  })
  const text = await response.text()
  let value = null
  try { value = text ? JSON.parse(text) : null } catch { value = text }
  if (!response.ok) throw new Error(value?.error?.message || value?.message || `HTTP ${response.status}`)
  return value
}

async function loadFavorites() {
  const query = encodeURIComponent(state.favoriteQuery)
  const displayLimit = state.favoriteQuery ? 300 : 2000
  const [result, indexResult] = await Promise.all([
    favoriteRequest(`/studio/favorites?q=${query}&limit=${displayLimit}`),
    state.favoriteQuery
      ? favoriteRequest('/studio/favorites?limit=2000')
      : Promise.resolve(null),
  ])
  state.favorites = Array.isArray(result?.items) ? result.items : []
  state.favoriteIndex = Array.isArray(indexResult?.items) ? indexResult.items : state.favorites
  state.favoriteTotal = Number(result?.allTotal || 0)
  renderFavoritesRail()
  syncFavoriteButtons()
}

function favoriteForSource(backend, threadId, turnId, itemId) {
  const key = favoriteSourceKey({ backend, threadId, turnId, itemId })
  return state.favoriteIndex.find((favorite) => favorite.scope !== 'selection' && favoriteSourceKey(favorite) === key) || null
}

function openFavoritesRail(scope = 'global') {
  state.favoriteScope = scope
  activateRightWorkspace('favorites')
  loadFavorites().catch(showError)
  setTimeout(() => $('#favorites-search').focus(), 30)
}

function closeFavoritesRail() {
  $('#favorites-rail').classList.add('hidden')
  if (state.activeRightWorkspace === 'favorites') state.activeRightWorkspace = null
  syncRightWorkspaceLaunchers()
  if ($('#annotation-rail').classList.contains('hidden')) {
    if (state.artifact) renderArtifact()
    else renderSessionMap()
  }
}

async function exportFavorites() {
  const response = await gatewayFetch('/studio/favorites/export', { cache: 'no-store' })
  if (!response.ok) throw new Error(await response.text() || `HTTP ${response.status}`)
  const url = URL.createObjectURL(await response.blob())
  const link = document.createElement('a')
  link.href = url
  link.download = 'codex-thread-studio-favorites.md'
  document.body.append(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  toast('Favorites exported')
}

function handleFavoritesSearch(event) {
  state.favoriteQuery = event.target.value.trim()
  clearTimeout(favoritesSearchTimer)
  favoritesSearchTimer = setTimeout(() => loadFavorites().catch(showError), 180)
}

function renderFavoritesRail() {
  const visibleFavorites = state.favoriteScope === 'session' && state.selectedId
    ? state.favorites.filter((favorite) => favorite.backend === state.backend && favorite.threadId === state.selectedId)
    : state.favorites
  const count = state.favoriteScope === 'session' ? visibleFavorites.length : state.favoriteTotal
  $('#favorites-title').textContent = t(state.favoriteScope === 'session' ? 'Session favorites' : 'Global favorites')
  $('#export-favorites').classList.toggle('hidden', state.favoriteScope !== 'global')
  $('#favorites-count').textContent = count
  $('#favorites-badge').textContent = count > 99 ? '99+' : count
  $('#favorites-badge').classList.toggle('hidden', count === 0)
  $('#favorites-search-summary').textContent = state.favoriteQuery
    ? t('Found {count} matching favorites', { count: visibleFavorites.length })
    : state.favoriteScope === 'session'
      ? t('{count} favorites in this session', { count })
      : t('{count} structured favorites across sessions', { count })
  const empty = visibleFavorites.length === 0
  $('#favorites-empty').classList.toggle('hidden', !empty)
  $('#favorites-list').classList.toggle('hidden', empty)
  $('#favorites-empty strong').textContent = state.favoriteQuery ? 'No matching results' : 'No favorites yet'
  $('#favorites-empty p').textContent = state.favoriteQuery
    ? 'Try keywords from the response, session name, or tags.'
    : 'Hover over an AI response and use its favorite button.'
  $('#favorites-list').innerHTML = visibleFavorites.map((favorite) => {
    const tags = favorite.tags?.length
      ? `<div class="favorite-card-tags">${favorite.tags.slice(0, 4).map((tag) => `<span>${escapeHtml(tag)}</span>`).join('')}</div>`
      : ''
    const question = favorite.questionSnippet
      ? `<p class="favorite-card-question"><span>Q</span>${escapeHtml(favorite.questionSnippet)}</p>`
      : ''
    return `<button class="favorite-card" type="button" data-favorite-id="${escapeHtml(favorite.id)}">
      <div class="favorite-card-top"><span class="favorite-backend-pill ${escapeHtml(favorite.backend)}">${escapeHtml(favorite.backend)}</span><time>${escapeHtml(formatFavoriteDate(favorite.createdAt))}</time></div>
      <strong>${escapeHtml(favorite.title)}</strong>
      ${question}
      <p class="favorite-card-answer">${escapeHtml(favorite.snippet)}</p>
      ${tags}
      <footer><span>${escapeHtml(favorite.threadTitle || t('Untitled session'))}</span><span>${escapeHtml(basename(favorite.projectPath))}</span></footer>
    </button>`
  }).join('')
  renderSessionFavoriteCount()
}

function renderSessionFavoriteCount() {
  const count = state.selectedId
    ? state.favoriteIndex.filter((favorite) => favorite.backend === state.backend && favorite.threadId === state.selectedId).length
    : 0
  const favoritesButton = $('#open-thread-favorites')
  setWorkspaceToolCount($('#thread-favorites-count'), count)
  const favoritesLabel = count ? `${t('Favorites')} · ${count}` : t('Favorites')
  favoritesButton.title = favoritesLabel
  favoritesButton.setAttribute('aria-label', favoritesLabel)
}

function setWorkspaceToolCount(badge, count) {
  if (!badge) return
  const normalized = Math.max(0, Number(count) || 0)
  badge.textContent = normalized > 99 ? '99+' : normalized
  badge.classList.toggle('hidden', normalized === 0)
}

function handleFavoriteListClick(event) {
  const card = event.target.closest('[data-favorite-id]')
  if (card) openFavoriteDetail(card.dataset.favoriteId).catch(showError)
}

function openFavoriteForMessage(turnId, itemId) {
  const turn = state.model.turns.find((candidate) => String(candidate.id) === String(turnId))
  const item = turn?.items?.find((candidate) => String(candidate.id) === String(itemId))
  const thread = selectedThread()
  const visibleText = item?.type === 'agentMessage' ? sessionMapVisibleText(item.text) : item?.text || ''
  if (!turn || !item || !thread || !visibleText.trim()) {
    toast('This response is not complete and cannot be favorited yet', 'error')
    return
  }
  state.favoriteEditMode = false
  state.pendingFavorite = {
    id: randomId(),
    scope: 'message',
    backend: state.backend,
    threadId: state.selectedId,
    threadTitle: threadTitle(thread),
    projectPath: thread.cwd || '',
    turnId: String(turn.id || ''),
    itemId: String(item.id || ''),
    title: autoFavoriteTitle(visibleText),
    question: questionForTurn(turn),
    content: visibleText.trim(),
    note: '',
    tags: [],
    createdAt: new Date().toISOString(),
  }
  populateFavoriteDialog(state.pendingFavorite)
}

function populateFavoriteDialog(favorite) {
  const editing = state.favoriteEditMode
  const resource = favorite.presentation === 'resource'
  $('#favorite-dialog-title').textContent = t(editing ? 'Edit favorite' : resource ? 'Save resource' : favorite.scope === 'selection' ? 'Favorite selection' : 'Favorite this response')
  $('#favorite-source-label').textContent = `${backendDescriptor(favorite.backend).name} · ${favorite.threadTitle || t('Untitled session')}`
  $('#favorite-preview-label').textContent = t(resource ? 'Resources' : 'AI response')
  $('#favorite-answer-length').textContent = t('{count} characters', { count: [...favorite.content].length.toLocaleString(getLocale()) })
  $('#favorite-answer-preview').innerHTML = renderMarkdown(favorite.content)
  $('#favorite-title').value = favorite.title || autoFavoriteTitle(favorite.content)
  $('#favorite-tags').value = (favorite.tags || []).join(', ')
  $('#favorite-note').value = favorite.note || ''
  $('#favorite-include-question').checked = Boolean(favorite.question)
  $('#favorite-question-option').classList.toggle('hidden', editing && !favorite.question)
  $('#save-favorite').textContent = t(editing ? 'Save changes' : 'Save favorite')
  $('#favorite-error').classList.add('hidden')
  renderFavoriteQuestionOption()
  $('#favorite-dialog').showModal()
  setTimeout(() => $('#favorite-title').focus(), 30)
}

function renderFavoriteQuestionOption() {
  const favorite = state.pendingFavorite
  if (!favorite) return
  const included = $('#favorite-include-question').checked && Boolean(favorite.question)
  $('#favorite-question-preview').classList.toggle('hidden', !included)
  $('#favorite-question-preview').textContent = included ? favorite.question : ''
}

function closeFavoriteDialog() {
  $('#favorite-dialog').close()
  state.pendingFavorite = null
  state.favoriteEditMode = false
  state.pendingSelection = null
  window.getSelection()?.removeAllRanges()
}

async function saveFavorite(event) {
  event.preventDefault()
  if (!state.pendingFavorite) return
  const favorite = {
    ...state.pendingFavorite,
    title: $('#favorite-title').value.trim(),
    question: $('#favorite-include-question').checked ? state.pendingFavorite.question : '',
    tags: normalizeFavoriteTags($('#favorite-tags').value),
    note: $('#favorite-note').value.trim(),
  }
  const error = $('#favorite-error')
  if (!favorite.title) {
    error.textContent = 'Enter a favorite title.'
    error.classList.remove('hidden')
    return
  }
  try {
    const updating = state.favoriteEditMode
    const saved = await favoriteRequest(
      updating ? `/studio/favorites/${encodeURIComponent(favorite.id)}` : '/studio/favorites',
      { method: updating ? 'PUT' : 'POST', body: JSON.stringify(favorite) },
    )
    closeFavoriteDialog()
    state.selectedFavorite = saved
    await loadFavorites()
    toast(updating ? 'Favorite updated' : 'Saved to global favorites')
    if (updating) await openFavoriteDetail(saved.id)
  } catch (requestError) {
    error.textContent = requestError.message
    error.classList.remove('hidden')
  }
}

async function openFavoriteDetail(id) {
  const favorite = await favoriteRequest(`/studio/favorites/${encodeURIComponent(id)}`)
  state.selectedFavorite = favorite
  $('#favorite-detail-backend').textContent = favorite.backend
  $('#favorite-detail-backend').className = `favorite-backend-pill ${favorite.backend}`
  $('#favorite-detail-title').textContent = favorite.title
  $('#favorite-detail-source').textContent = `${favorite.threadTitle || t('Untitled session')} · ${favorite.projectPath || t('Project directory not recorded')} · ${formatFavoriteDate(favorite.createdAt)}`
  $('#favorite-detail-question-section').classList.toggle('hidden', !favorite.question)
  $('#favorite-detail-question').textContent = favorite.question || ''
  $('#favorite-detail-answer').innerHTML = renderMarkdown(favorite.content)
  $('#favorite-detail-note-section').classList.toggle('hidden', !favorite.note)
  $('#favorite-detail-note').textContent = favorite.note || ''
  $('#favorite-detail-tags').innerHTML = (favorite.tags || []).map((tag) => `<span>${escapeHtml(tag)}</span>`).join('')
  $('#favorite-detail-dialog').showModal()
}

function closeFavoriteDetail() {
  $('#favorite-detail-dialog').close()
  state.selectedFavorite = null
}

async function copySelectedFavorite() {
  if (!state.selectedFavorite) return
  await navigator.clipboard.writeText(favoriteCopyText(state.selectedFavorite, t))
  toast('Favorite copied')
}

function editSelectedFavorite() {
  if (!state.selectedFavorite) return
  const favorite = { ...state.selectedFavorite, tags: [...(state.selectedFavorite.tags || [])] }
  $('#favorite-detail-dialog').close()
  state.favoriteEditMode = true
  state.pendingFavorite = favorite
  populateFavoriteDialog(favorite)
}

async function deleteSelectedFavorite() {
  const favorite = state.selectedFavorite
  if (!favorite || !confirm(t('Delete favorite “{title}”?', { title: favorite.title }))) return
  await favoriteRequest(`/studio/favorites/${encodeURIComponent(favorite.id)}`, { method: 'DELETE' })
  closeFavoriteDetail()
  await loadFavorites()
  toast('Favorite deleted')
}

async function openSelectedFavoriteSource() {
  const favorite = state.selectedFavorite
  if (!favorite) return
  closeFavoriteDetail()
  closeFavoritesRail()
  if (state.backend !== favorite.backend) {
    await switchBackend(favorite.backend)
    await waitFor(() => state.ready, 12_000)
  }
  if (!state.threads.some((thread) => thread.id === favorite.threadId)) {
    if (state.ready) await loadThreads()
  }
  if (!state.threads.some((thread) => thread.id === favorite.threadId)) {
    throw new Error('The source session is not in the list and may be archived or deleted. The favorite remains intact.')
  }
  await selectThread(favorite.threadId, { force: true })
  const element = renderedItem(favorite.turnId, favorite.itemId)
  if (!element) {
    toast('Returned to the source session, but the original message anchor was not found', 'error')
    return
  }
  transcriptScrollFollower.pause()
  element.classList.add('favorite-source-highlight')
  element.scrollIntoView({ behavior: 'smooth', block: 'center' })
  setTimeout(() => element.classList.remove('favorite-source-highlight'), 2400)
}

function syncFavoriteButtons() {
  $('#transcript')?.querySelectorAll('[data-favorite-message]').forEach((button) => {
    const item = button.closest('[data-turn-id][data-item-id]')
    const favorite = item && favoriteForSource(state.backend, state.selectedId, item.dataset.turnId, item.dataset.itemId)
    button.classList.toggle('active', Boolean(favorite))
    button.setAttribute('aria-pressed', String(Boolean(favorite)))
    button.title = favorite ? 'Favorited; click to view' : 'Favorite this response'
    button.setAttribute('aria-label', button.title)
    const accessibleLabel = button.querySelector('b')
    if (accessibleLabel) accessibleLabel.textContent = favorite ? 'Favorited' : 'Favorites'
    item?.classList.toggle('favorited', Boolean(favorite))
  })
}

function formatFavoriteDate(value) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return formatLocalizedDate(date, { year: 'numeric', month: 'short', day: 'numeric' })
}

  function basename(path) {
    return String(path || '').split(/[\\/]/u).filter(Boolean).at(-1) || ''
  }

  function escapeHtml(value) {
    return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
    })[character])
  }

  return {
    bind,
    buildPrompt: buildAnnotationPrompt,
    captureArtifactSelection,
    captureTranscriptSelection,
    closeAnnotations: closeAnnotationRail,
    closeFavorites: closeFavoritesRail,
    currentAnnotations,
    favoriteForSource,
    hideSelection: hideSelectionPopover,
    loadFavorites,
    openAnnotationFromSelection,
    openAnnotations: openAnnotationRail,
    openCommentForSelection,
    openFavoriteDetail,
    openFavoriteForMessage,
    openFavoriteForResource,
    openFavorites: openFavoritesRail,
    positionSelection: positionSelectionPopover,
    renderAnnotations: renderAnnotationRail,
    renderComposerContext: renderComposerReviewContext,
    renderFavorites: renderFavoritesRail,
    renderSessionFavoriteCount,
    setSelection,
    syncFavoriteButtons,
  }
}
