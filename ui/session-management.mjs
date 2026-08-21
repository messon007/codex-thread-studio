import { BACKEND_IDS, backendDescriptor, isCodexBackend } from './backends.mjs'
import { getLocale, t } from './i18n.mjs'
import { catalogListParams } from './session-catalog.mjs'
import {
  filterSessionOccurrences,
  isUnsupportedOccurrenceSearchError,
  localSessionOccurrences,
  normalizeRemoteSessionOccurrences,
} from './session-search.mjs'
import {
  catalogTimestamp,
  groupCatalogEntries,
  isSessionDirectoryHidden,
  threadCatalogKey,
} from './thread-catalog.mjs'

export function createSessionLibraryState() {
  return {
    open: false,
    loading: false,
    loaded: false,
    query: '',
    entries: [],
    selected: null,
    error: '',
    errorsByBackend: {},
    generation: 0,
    nextCursors: {},
    returnBackend: null,
    returnId: null,
  }
}

export function createThreadSearchState() {
  return {
    open: false,
    query: '',
    type: 'all',
    entries: [],
    selected: -1,
    loading: false,
    generation: 0,
  }
}

export function createSessionManagementUI({
  state,
  backend,
  catalog,
  view,
  searchView,
  notify = () => {},
  reportError = console.error,
}) {
  const { requestCodexBackend, rpc, switchBackend, waitFor } = backend
  const { selectThread, loadThreads, installBackendCatalog, selectedThread, threadTitle } = catalog
  const {
    createViewModel,
    hydrateThread,
    closeActionMenus,
    closeWorkspacePeerRails,
    renderThreadList,
    renderWorkspace,
    renderTranscript,
  } = view
  const element = (id) => document.getElementById(id)
  const occurrenceSearchSupport = new Map()
  let searchTimer = null

  function bind() {
    element('open-archived-sessions')?.addEventListener('click', () => archive.open().catch(reportError))
    element('close-archived-sessions')?.addEventListener('click', () => archive.close().catch(reportError))
    element('restore-archived-session')?.addEventListener('click', () => archive.restore().catch(reportError))
    element('thread-search')?.addEventListener('input', (event) => {
      if (state.sessionLibrary.open) state.sessionLibrary.query = event.target.value.trim().toLowerCase()
      else state.search = event.target.value.trim().toLowerCase()
      renderThreadList()
    })
    element('open-thread-search')?.addEventListener('click', search.open)
    element('close-thread-search')?.addEventListener('click', () => search.close())
    element('thread-content-search-input')?.addEventListener('input', handleSearchInput)
    element('thread-content-search-input')?.addEventListener('focus', () => search.render())
    element('thread-content-search-input')?.addEventListener('keydown', handleSearchKeydown)
    element('thread-content-search-prev')?.addEventListener('click', () => navigateSearch(-1))
    element('thread-content-search-next')?.addEventListener('click', () => navigateSearch(1))
    element('thread-content-search-type')?.addEventListener('change', (event) => {
      state.threadSearch.type = event.target.value
      state.threadSearch.selected = filteredSearchEntries().length ? 0 : -1
      search.render()
    })
    element('thread-content-search-results')?.addEventListener('mousedown', (event) => event.preventDefault())
    element('thread-content-search-results')?.addEventListener('click', handleSearchResultClick)
  }

  const archive = {
    isOpen() {
      return state.sessionLibrary.open
    },

    async open() {
      closeActionMenus()
      search.close()
      const library = state.sessionLibrary
      if (!library.open) {
        library.returnBackend = state.backend
        library.returnId = state.selectedId
      }
      library.open = true
      element('archived-sidebar-header')?.classList.remove('hidden')
      document.querySelector('.thread-filters')?.classList.add('hidden')
      const input = element('thread-search')
      input.value = library.query
      input.placeholder = t('Search archived sessions')
      renderThreadList()
      if (!library.loaded) await archive.load()
      else if (Object.keys(library.errorsByBackend).length) {
        await archive.load({ backends: Object.keys(library.errorsByBackend) })
      }
    },

    async close({ restoredId = null, restoredBackend = null } = {}) {
      const library = state.sessionLibrary
      const returnBackend = restoredBackend || library.returnBackend || state.backend
      const returnId = restoredId || library.returnId || state.selectedByBackend[returnBackend]
      library.open = false
      library.selected = null
      library.returnBackend = null
      library.returnId = null
      element('archived-sidebar-header')?.classList.add('hidden')
      document.querySelector('.thread-filters')?.classList.remove('hidden')
      const input = element('thread-search')
      input.value = state.search
      input.placeholder = t('Search sessions or paths')
      if (returnBackend !== state.backend) {
        await switchBackend(returnBackend, { selectedId: returnId || undefined })
        await waitFor(() => state.backend === returnBackend && state.ready, 15_000)
        if (returnId) await selectThread(returnId, { force: true, backend: returnBackend }).catch(reportError)
        return
      }
      state.selectedId = returnId || null
      state.model = createViewModel()
      renderThreadList()
      renderWorkspace()
      renderTranscript()
      if (returnId && state.threads.some((thread) => thread.id === returnId)) await selectThread(returnId, { force: true })
      else await loadThreads()
    },

    async load({ append = false, backends = null } = {}) {
      const library = state.sessionLibrary
      const generation = ++library.generation
      library.loading = true
      renderThreadList()
      const availableBackends = BACKEND_IDS.filter(isCodexBackend)
      const requestedBackends = (Array.isArray(backends) ? backends : availableBackends)
        .filter((backend, index, values) => availableBackends.includes(backend) && values.indexOf(backend) === index)
        .filter((backend) => !append || library.nextCursors[backend])
      if (!requestedBackends.length) {
        library.loading = false
        renderThreadList()
        return
      }
      const results = await Promise.all(requestedBackends.map(async (backend) => {
        const cursor = append ? library.nextCursors[backend] : null
        try {
          const result = await requestCodexBackend(backend, 'thread/list', catalogListParams('codex', {
            archived: true,
            limit: 100,
            cursor,
            sortKey: 'updated_at',
            sortDirection: 'desc',
          }))
          return { backend, status: 'fulfilled', data: Array.isArray(result?.data) ? result.data : [], nextCursor: result?.nextCursor || null }
        } catch (error) {
          return { backend, status: 'rejected', error }
        }
      }))
      if (generation !== library.generation) return
      const successful = results.filter((result) => result.status === 'fulfilled')
      const failed = results.filter((result) => result.status === 'rejected')
      const entries = successful.flatMap(({ backend, data }) => data.map((thread) => ({ backend, thread: { ...thread, archived: true } })))
      const refreshedBackends = new Set(successful.map(({ backend }) => backend))
      const existing = append
        ? library.entries
        : library.entries.filter(({ backend }) => !refreshedBackends.has(backend))
      const byKey = new Map([...existing, ...entries].map((entry) => [threadCatalogKey(entry.backend, entry.thread.id), entry]))
      library.entries = [...byKey.values()].sort((left, right) => threadUpdatedAt(right.thread) - threadUpdatedAt(left.thread))
      const nextCursors = { ...library.nextCursors }
      for (const { backend, nextCursor } of successful) nextCursors[backend] = nextCursor
      library.nextCursors = nextCursors
      const errorsByBackend = { ...library.errorsByBackend }
      for (const { backend } of successful) delete errorsByBackend[backend]
      for (const { backend, error } of failed) errorsByBackend[backend] = error?.message || String(error)
      library.errorsByBackend = errorsByBackend
      library.error = Object.entries(errorsByBackend)
        .map(([backend, message]) => `${backendDescriptor(backend).name}: ${message}`)
        .join(' · ')
      library.loading = false
      library.loaded = true
      archive.renderBadge()
      renderThreadList()
    },

    renderList() {
      const library = state.sessionLibrary
      const list = element('thread-list')
      if (library.loading && !library.entries.length) {
        list.innerHTML = `<div class="list-empty">${t('Loading archived sessions…')}</div>`
        return
      }
      if (library.error && !library.entries.length) {
        list.innerHTML = `<div class="list-empty error">${escapeHtml(library.error)}<button class="subtle-button compact" type="button" data-retry-archived>${t('Retry')}</button></div>`
        bindArchiveRetry(list)
        return
      }
      const entries = filteredArchiveEntries()
      if (!entries.length) {
        list.innerHTML = `<div class="list-empty">${t(library.query ? 'No matching archived sessions' : 'No archived sessions')}</div>`
        return
      }
      const errorBanner = library.error
        ? `<div class="archive-load-error">${escapeHtml(library.error)}<button class="subtle-button compact" type="button" data-retry-archived>${t('Retry')}</button></div>`
        : ''
      const renderRow = ({ backend, thread }) => {
        const active = library.selected?.backend === backend && library.selected?.thread?.id === thread.id
        const updated = threadUpdatedAt(thread)
        const date = updated ? new Intl.DateTimeFormat(getLocale(), { dateStyle: 'medium' }).format(new Date(updated)) : ''
        return `<button class="thread-row archived${active ? ' active' : ''}" data-archived-thread-id="${escapeHtml(thread.id)}" data-backend="${escapeHtml(backend)}">
          <span class="archived-thread-mark" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 8h16v11H4zM3 4h18v4H3zM9 12h6"/></svg></span>
          <span class="thread-copy"><strong>${escapeHtml(threadTitle(thread))}</strong><small data-no-i18n title="${escapeHtml(thread.cwd || t('Project directory not recorded'))}">${escapeHtml(thread.cwd || t('Project directory not recorded'))}</small>${date ? `<small>${t('Archived {date}', { date })}</small>` : ''}</span>
          <span class="backend-tag ${escapeHtml(backend)}" title="${escapeHtml(backendDescriptor(backend).name)}">${escapeHtml(backendDescriptor(backend).tag)}</span>
        </button>`
      }
      list.innerHTML = errorBanner + groupCatalogEntries(entries).map(({ cwd, name, entries: groupEntries }) => `<section class="thread-group" data-group-path="${escapeHtml(cwd)}">
        <div class="thread-group-heading static" data-no-i18n title="${escapeHtml(cwd || t('Project directory not recorded'))}"><strong>${escapeHtml(name || t('Other sessions'))}</strong><span>${groupEntries.length}</span></div>
        <div class="thread-group-sessions">${groupEntries.map(renderRow).join('')}</div>
      </section>`).join('')
      if (Object.values(library.nextCursors).some(Boolean)) {
        list.insertAdjacentHTML('beforeend', `<button class="load-more-archived" type="button" data-load-more-archived>${t('Load more')}</button>`)
      }
      list.querySelectorAll('[data-archived-thread-id]').forEach((row) => row.addEventListener('click', () => archive.openSession(row.dataset.backend, row.dataset.archivedThreadId).catch(reportError)))
      list.querySelector('[data-load-more-archived]')?.addEventListener('click', () => archive.load({ append: true }).catch(reportError))
      bindArchiveRetry(list)
    },

    renderBadge() {
      const badge = element('archived-sessions-badge')
      const count = state.sessionLibrary.entries.length
      badge.textContent = count
      badge.classList.toggle('hidden', !state.sessionLibrary.loaded || count === 0)
    },

    async openSession(backend, threadId) {
      const library = state.sessionLibrary
      const entry = library.entries.find((candidate) => candidate.backend === backend && candidate.thread.id === threadId)
      if (!entry) return
      library.selected = entry
      renderThreadList()
      if (backend !== state.backend) {
        await switchBackend(backend)
        await waitFor(() => state.backend === backend && state.ready, 15_000)
      }
      if (!library.open || library.selected !== entry) return
      const result = await rpc('thread/read', { threadId, includeTurns: true })
      if (!library.open || library.selected !== entry) return
      entry.thread = { ...entry.thread, ...(result.thread || {}), archived: true, turns: undefined }
      state.selectedId = threadId
      state.model = createViewModel()
      hydrateThread(state.model, result.thread)
      closeWorkspacePeerRails()
      renderThreadList()
      renderWorkspace()
      renderTranscript()
      element('native-connection').textContent = t('Archived')
    },

    async restore() {
      const library = state.sessionLibrary
      const selected = library.selected
      if (!selected || selected.backend !== state.backend || state.selectedId !== selected.thread.id) return
      const button = element('restore-archived-session')
      button.disabled = true
      try {
        const result = await rpc('thread/unarchive', { threadId: selected.thread.id })
        const restored = { ...selected.thread, ...(result?.thread || {}), archived: false, turns: undefined }
        const catalog = state.threadsByBackend[selected.backend] || []
        installBackendCatalog(selected.backend, [restored, ...catalog.filter((thread) => thread.id !== restored.id)])
        library.entries = library.entries.filter((entry) => !(entry.backend === selected.backend && entry.thread.id === selected.thread.id))
        library.loaded = false
        state.selectedByBackend[selected.backend] = restored.id
        archive.renderBadge()
        await archive.close({ restoredId: restored.id, restoredBackend: selected.backend })
        notify(t('Session restored'))
      } finally {
        if (button.isConnected) button.disabled = false
      }
    },

    markStale({ reload = false } = {}) {
      state.sessionLibrary.loaded = false
      archive.renderBadge()
      if (reload && state.sessionLibrary.open) archive.load().catch(reportError)
    },

    remove(backend, threadId) {
      const library = state.sessionLibrary
      library.entries = library.entries.filter((entry) => !(entry.backend === backend && entry.thread.id === threadId))
      if (library.selected?.backend === backend && library.selected.thread.id === threadId) library.selected = null
      archive.renderBadge()
    },

    isPreview() {
      const selected = state.sessionLibrary.selected
      return Boolean(state.sessionLibrary.open && selected?.backend === state.backend && selected.thread?.id === state.selectedId)
    },

    selectedThread() {
      const selected = state.sessionLibrary.selected
      return archive.isPreview() ? selected.thread : null
    },
  }

  const search = {
    open() {
      if (!selectedThread()) return
      state.threadSearch.open = true
      element('thread-content-search')?.classList.remove('hidden')
      element('open-thread-search')?.setAttribute('aria-pressed', 'true')
      element('thread-content-search-input').value = state.threadSearch.query
      element('thread-content-search-type').value = state.threadSearch.type
      search.render()
      requestAnimationFrame(() => {
        const input = element('thread-content-search-input')
        input.focus()
        input.select()
      })
    },

    close({ clear = false } = {}) {
      clearTimeout(searchTimer)
      state.threadSearch.open = false
      state.threadSearch.loading = false
      if (clear) {
        state.threadSearch.query = ''
        state.threadSearch.entries = []
        state.threadSearch.selected = -1
        element('thread-content-search-input').value = ''
      }
      element('thread-content-search')?.classList.add('hidden')
      element('open-thread-search')?.setAttribute('aria-pressed', 'false')
      search.hideResults()
      clearSearchTarget()
    },

    hideResults() {
      element('thread-content-search-results')?.classList.add('hidden')
    },

    async perform() {
      const query = state.threadSearch.query
      const threadId = state.selectedId
      const backend = state.backend
      if (!query || !threadId) return
      const generation = ++state.threadSearch.generation
      const isCurrent = () => generation === state.threadSearch.generation
        && query === state.threadSearch.query
        && threadId === state.selectedId
        && backend === state.backend
      let entries
      if (isCodexBackend(backend) && occurrenceSearchSupport.get(backend) !== false) {
        try {
          const result = await rpc('thread/searchOccurrences', { threadId, searchTerm: query, limit: 100 })
          if (!isCurrent()) return
          occurrenceSearchSupport.set(backend, true)
          entries = normalizeRemoteSessionOccurrences(result?.data, state.model)
        } catch (error) {
          if (!isUnsupportedOccurrenceSearchError(error)) throw error
          occurrenceSearchSupport.set(backend, false)
          console.debug('Backend session search is unavailable; using the loaded session model', error)
        }
      }
      if (!entries) {
        if (!isCurrent()) return
        entries = localSessionOccurrences(state.model, query)
      }
      if (!isCurrent()) return
      state.threadSearch.entries = entries
      state.threadSearch.loading = false
      state.threadSearch.selected = entries.length ? 0 : -1
      search.render()
    },

    render(error = '') {
      const panel = element('thread-content-search')
      if (!state.threadSearch.open || panel.classList.contains('hidden')) return
      const entries = filteredSearchEntries()
      const summary = element('thread-content-search-summary')
      summary.textContent = error
        ? t('Search failed')
        : state.threadSearch.loading
          ? t('Searching…')
          : state.threadSearch.query
            ? t('{count} matches', { count: entries.length })
            : ''
      element('thread-content-search-prev').disabled = !entries.length
      element('thread-content-search-next').disabled = !entries.length
      const results = element('thread-content-search-results')
      if (!state.threadSearch.query || state.threadSearch.loading || error) {
        results.innerHTML = error ? `<div class="composer-menu-empty">${escapeHtml(error)}</div>` : ''
        results.classList.toggle('hidden', !error)
        return
      }
      if (!entries.length) {
        results.innerHTML = `<div class="composer-menu-empty">${t('No matches in this session')}</div>`
        results.classList.remove('hidden')
        return
      }
      results.innerHTML = entries.map((entry, index) => {
        const selected = index === state.threadSearch.selected
        const turnNumber = Math.max(1, Number(entry.turnIndex) + 1)
        return `<div id="thread-search-option-${index}" class="thread-content-search-result${selected ? ' selected' : ''}" role="option" aria-selected="${selected}" data-thread-search-index="${index}">
          <strong>${escapeHtml(searchTypeLabel(entry.type))} · ${t('Turn {index}', { index: turnNumber })}</strong>
          <small>${highlightSearchSnippet(entry)}</small>
        </div>`
      }).join('')
      results.classList.remove('hidden')
      element('thread-content-search-input').setAttribute('aria-activedescendant', `thread-search-option-${Math.max(0, state.threadSearch.selected)}`)
    },
  }

  function filteredArchiveEntries() {
    const library = state.sessionLibrary
    return library.entries.filter(({ backend, thread }) => {
      if (isSessionDirectoryHidden(thread.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore)) return false
      if (!library.query) return true
      return [threadTitle(thread), thread.cwd, thread.id, backendDescriptor(backend).name, backendDescriptor(backend).tag]
        .filter(Boolean).join(' ').toLowerCase().includes(library.query)
    })
  }

  function bindArchiveRetry(container) {
    container.querySelector('[data-retry-archived]')?.addEventListener('click', () => {
      const failed = Object.keys(state.sessionLibrary.errorsByBackend)
      archive.load({ backends: failed.length ? failed : null }).catch(reportError)
    })
  }

  function handleSearchInput(event) {
    state.threadSearch.query = event.target.value.trim()
    state.threadSearch.selected = -1
    clearTimeout(searchTimer)
    if (!state.threadSearch.query) {
      state.threadSearch.entries = []
      state.threadSearch.loading = false
      search.render()
      return
    }
    state.threadSearch.loading = true
    search.render()
    searchTimer = setTimeout(() => search.perform().catch((error) => {
      state.threadSearch.loading = false
      state.threadSearch.entries = []
      search.render(error.message)
    }), 180)
  }

  function filteredSearchEntries() {
    return filterSessionOccurrences(state.threadSearch.entries, state.threadSearch.type)
  }

  function handleSearchKeydown(event) {
    const entries = filteredSearchEntries()
    if (event.key === 'Escape') {
      event.preventDefault()
      const results = element('thread-content-search-results')
      if (!results.classList.contains('hidden')) {
        search.hideResults()
        return
      }
      search.close()
      return
    }
    if (!entries.length) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const direction = event.key === 'ArrowDown' ? 1 : -1
      state.threadSearch.selected = (state.threadSearch.selected + direction + entries.length) % entries.length
      search.render()
      document.querySelector(`[data-thread-search-index="${state.threadSearch.selected}"]`)?.scrollIntoView({ block: 'nearest' })
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      if (event.shiftKey) navigateSearch(-1)
      else if (state.threadSearch.selected >= 0) selectSearchEntry(state.threadSearch.selected)
      else navigateSearch(1)
    }
  }

  function handleSearchResultClick(event) {
    const option = event.target.closest('[data-thread-search-index]')
    if (option) selectSearchEntry(Number(option.dataset.threadSearchIndex))
  }

  function navigateSearch(direction) {
    const entries = filteredSearchEntries()
    if (!entries.length) return
    state.threadSearch.selected = (state.threadSearch.selected + direction + entries.length) % entries.length
    selectSearchEntry(state.threadSearch.selected)
  }

  function selectSearchEntry(index) {
    const entries = filteredSearchEntries()
    const entry = entries[index]
    if (!entry) return
    state.threadSearch.selected = index
    searchView.showTurn(entry.turnId)
    renderTranscript()
    let target = searchView.renderedItem(entry.turnId, entry.itemId)
    if (!target && entry.type === 'activity') {
      const block = searchView.activityBlocks(entry.turnId).find((candidate) => candidate.sourceItemIds.includes(String(entry.itemId || '')))
      target = searchView.renderedActivity(entry.turnId, block?.id)
      if (target) {
        target.open = true
        searchView.hydrateActivity(target)
      }
    }
    if (!target) target = [...element('transcript').querySelectorAll('.turn[data-turn-id]')].find((turn) => turn.dataset.turnId === String(entry.turnId || ''))
    clearSearchTarget()
    if (target) {
      target.classList.add('session-search-target')
      searchView.pauseFollowing()
      const transcript = element('transcript')
      const top = transcript.scrollTop + target.getBoundingClientRect().top - transcript.getBoundingClientRect().top - 18
      transcript.scrollTo({ top: Math.max(0, top), behavior: 'smooth' })
    }
    search.hideResults()
    const current = state.threadSearch.selected >= 0 ? state.threadSearch.selected + 1 : 0
    element('thread-content-search-summary').textContent = t('{current} of {total}', { current, total: entries.length })
  }

  function clearSearchTarget() {
    element('transcript')?.querySelectorAll('.session-search-target').forEach((target) => target.classList.remove('session-search-target'))
  }

  return { bind, archive, search }
}

function threadUpdatedAt(thread) {
  return catalogTimestamp(thread?.updatedAt || thread?.updated_at || thread?.createdAt)
}

function searchTypeLabel(type) {
  if (type === 'user') return t('User')
  if (type === 'activity') return t('Progress and activity')
  return t('Assistant')
}

function highlightSearchSnippet(entry) {
  const snippet = String(entry.snippet || '')
  const start = Math.max(0, Math.min(snippet.length, Number(entry.snippetMatchRange?.start || 0)))
  const end = Math.max(start, Math.min(snippet.length, Number(entry.snippetMatchRange?.end || start)))
  return `${escapeHtml(snippet.slice(0, start))}<mark>${escapeHtml(snippet.slice(start, end))}</mark>${escapeHtml(snippet.slice(end))}`
}

function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  })[character])
}
