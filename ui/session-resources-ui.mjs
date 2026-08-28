import { buildSessionResourceIndex, resourceGroup, resourceIcon, sessionResourceRevision } from './session-resources.mjs'

export function createSessionResourcesUI({
  getModel,
  getThread,
  getBackend,
  activate,
  deactivate,
  openResource,
  openSource,
  favoriteResource,
  translate = (value) => value,
  notify = () => {},
  buildIndex = buildSessionResourceIndex,
  scheduleIdle = scheduleLazyResourceScan,
}) {
  const states = new Map()
  let developerContext = null
  let pendingLazyScan = null
  const element = (id) => document.getElementById(id)
  const currentThread = () => developerContext?.thread || getThread?.() || null
  const currentModel = () => developerContext?.model || getModel?.() || null
  const currentBackend = () => developerContext?.backend || getBackend?.() || 'codex'

  function currentKey() {
    const thread = currentThread()
    return thread?.id ? `${currentBackend()}:${thread.id}` : ''
  }

  function stateForCurrent() {
    const key = currentKey()
    const thread = currentThread()
    if (!key || !thread) return null
    if (!states.has(key)) {
      states.set(key, {
        key,
        index: buildIndex(null),
        query: '',
        filter: 'all',
        selectedId: '',
        built: false,
        builtRevision: '',
        builtModel: null,
        builtLatestTurn: null,
        dirty: true,
      })
    }
    return states.get(key)
  }

  function bind() {
    element('open-thread-resources')?.addEventListener('click', open)
    element('close-resources')?.addEventListener('click', close)
    element('refresh-resources')?.addEventListener('click', () => {
      rebuild()
      notify(translate('Resource index refreshed'))
    })
    element('resources-search')?.addEventListener('input', (event) => {
      const state = stateForCurrent()
      if (!state) return
      state.query = event.target.value
      render()
    })
    element('resources-filters')?.addEventListener('click', (event) => {
      const button = event.target.closest('[data-resource-filter]')
      const state = stateForCurrent()
      if (!button || !state) return
      state.filter = button.dataset.resourceFilter
      render()
    })
    element('resources-list')?.addEventListener('click', handleListClick)
  }

  function sync({ rebuild: shouldRebuild = false, contentChanged = false } = {}) {
    const state = stateForCurrent()
    const button = element('open-thread-resources')
    if (button) button.disabled = !state
    if (!state) {
      cancelLazyScan()
      close()
      syncLauncher(0)
      return
    }
    if (shouldRebuild || contentChanged || sourceChanged(state)) state.dirty = true
    if (!isOpen()) {
      // Keep the last known badge visible immediately, then refresh it after
      // the transcript has painted. Only the latest Turn is indexed.
      syncLauncher(knownCount(state))
      if (state.dirty) scheduleLazyRebuild(state)
      else cancelLazyScan()
      return
    }
    cancelLazyScan()
    const revision = currentRevision()
    state.dirty = state.builtRevision !== revision
    if (shouldRebuild || (isOpen() && state.dirty)) rebuild({ revision })
    else {
      syncLauncher(state.built ? state.index.counts().all : 0)
      render()
    }
  }

  function syncSelection() {
    const state = stateForCurrent()
    const button = element('open-thread-resources')
    if (button) button.disabled = !state
    if (!state) {
      cancelLazyScan()
      close()
      syncLauncher(0)
      return
    }
    if (isOpen()) sync()
    else {
      if (sourceChanged(state)) state.dirty = true
      syncLauncher(knownCount(state))
      if (state.dirty) scheduleLazyRebuild(state)
      else cancelLazyScan()
    }
  }

  function rebuild({ revision = currentRevision() } = {}) {
    cancelLazyScan()
    const state = stateForCurrent()
    const thread = currentThread()
    if (!state || !thread) return null
    const model = currentModel()
    state.index = buildIndex(model, {
      backend: currentBackend(),
      threadId: thread.id,
      root: thread.cwd || '',
    })
    state.built = true
    state.builtRevision = revision
    state.builtModel = model
    state.builtLatestTurn = latestTurn(model)
    state.dirty = false
    if (state.selectedId && !state.index.resourcesById.has(state.selectedId)) state.selectedId = ''
    syncLauncher(state.index.counts().all)
    if (isOpen()) render()
    return state.index
  }

  function open() {
    if (!stateForCurrent()) return
    cancelLazyScan()
    activate?.('resources')
    element('resources-rail')?.classList.remove('hidden')
    sync()
    setTimeout(() => element('resources-search')?.focus(), 30)
  }

  function close() {
    element('resources-rail')?.classList.add('hidden')
    deactivate?.('resources')
    developerContext = null
    const state = stateForCurrent()
    syncLauncher(knownCount(state))
    if (state?.dirty) scheduleLazyRebuild(state)
  }

  function isOpen() {
    return !element('resources-rail')?.classList.contains('hidden')
  }

  function render() {
    const state = stateForCurrent()
    const thread = currentThread()
    if (!state || !thread) return
    const counts = state.index.counts()
    element('resources-path').textContent = thread.cwd || thread.id
    element('resources-count').textContent = String(counts.all)
    element('resources-search').value = state.query
    for (const button of element('resources-filters')?.querySelectorAll('[data-resource-filter]') || []) {
      const filter = button.dataset.resourceFilter
      const count = counts[filter] || 0
      button.classList.toggle('active', filter === state.filter)
      button.setAttribute('aria-selected', String(filter === state.filter))
      button.querySelector('span').textContent = String(count)
    }
    const resources = state.index.resources({ query: state.query, kind: state.filter })
    const list = element('resources-list')
    const empty = element('resources-empty')
    empty.classList.toggle('hidden', resources.length > 0)
    list.classList.toggle('hidden', resources.length === 0)
    list.innerHTML = resources.map((resource) => renderResourceCard(resource, state)).join('')
    element('resources-summary').textContent = translate('{resources} resources · {occurrences} references')
      .replace('{resources}', String(counts.all)).replace('{occurrences}', String(counts.occurrences))
    element('resources-local-state').textContent = translate('Latest turn only')
  }

  function renderResourceCard(resource, state) {
    const occurrences = state.index.occurrences(resource.id)
    const selected = state.selectedId === resource.id
    const group = resourceGroup(resource.kind)
    const target = resource.target?.url || resource.target?.path || resource.raw
    const location = resource.target?.line ? `${target}:${resource.target.line}${resource.target.column ? `:${resource.target.column}` : ''}` : target
    const label = resource.state === 'blocked' ? translate('Blocked') : group === 'web' ? translate('Web') : group === 'code' ? translate('Code') : translate('Files')
    const occurrenceRows = selected ? `<div class="resource-occurrences">${occurrences.map((occurrence, index) => `<button type="button" data-resource-source="${escapeHtml(occurrence.id)}"><span>${sourceLabel(occurrence, index, translate)}</span><small>${escapeHtml(occurrence.excerpt || '')}</small></button>`).join('')}</div>` : ''
    return `<article class="resource-card${selected ? ' selected' : ''}${resource.state === 'blocked' ? ' blocked' : ''}" data-resource-id="${escapeHtml(resource.id)}">
      <button class="resource-card-main" type="button" data-resource-select="${escapeHtml(resource.id)}">
        <span class="resource-kind-icon ${escapeHtml(resourceIcon(resource.kind))}" aria-hidden="true">${resourceKindSvg(resource.kind)}</span>
        <span class="resource-card-copy"><strong>${escapeHtml(resource.display)}</strong><small>${escapeHtml(location)}</small></span>
        <span class="resource-kind-pill">${escapeHtml(label)} · ${occurrences.length}</span>
      </button>
      ${resource.state === 'blocked' ? `<p class="resource-blocked-reason">${escapeHtml(translate(resource.reason || 'This resource was blocked by the security policy'))}</p>` : ''}
      <div class="resource-card-actions">
        <button class="resource-open" type="button" data-resource-open="${escapeHtml(resource.id)}" ${resource.state === 'blocked' ? 'disabled' : ''}>${escapeHtml(openLabel(resource, translate))}</button>
        <button type="button" data-resource-source="${escapeHtml(occurrences.at(-1)?.id || '')}">${escapeHtml(translate('Go to message'))}</button>
        <button type="button" data-resource-copy="${escapeHtml(resource.id)}">${escapeHtml(translate('Copy address'))}</button>
        <button class="resource-favorite" type="button" data-resource-favorite="${escapeHtml(resource.id)}" title="${escapeHtml(translate('Save resource'))}" aria-label="${escapeHtml(translate('Save resource'))}">${favoriteSvg()}</button>
      </div>${occurrenceRows}
    </article>`
  }

  async function handleListClick(event) {
    const state = stateForCurrent()
    if (!state) return
    const openButton = event.target.closest('[data-resource-open]')
    if (openButton) {
      const resource = state.index.resourcesById.get(openButton.dataset.resourceOpen)
      if (resource) {
        try { await openResource?.(resource) } catch (error) { notify(error?.message || translate('Unable to open resource'), 'error') }
      }
      return
    }
    const sourceButton = event.target.closest('[data-resource-source]')
    if (sourceButton?.dataset.resourceSource) {
      const occurrence = state.index.occurrencesById.get(sourceButton.dataset.resourceSource)
      if (occurrence) openSource?.(occurrence)
      return
    }
    const favoriteButton = event.target.closest('[data-resource-favorite]')
    if (favoriteButton) {
      const resource = state.index.resourcesById.get(favoriteButton.dataset.resourceFavorite)
      const occurrence = resource && state.index.occurrences(resource.id).at(-1)
      if (resource && occurrence) favoriteResource?.(resource, occurrence)
      return
    }
    const copyButton = event.target.closest('[data-resource-copy]')
    if (copyButton) {
      const resource = state.index.resourcesById.get(copyButton.dataset.resourceCopy)
      if (!resource) return
      try {
        await navigator.clipboard.writeText(resource.target?.url || resource.raw)
        notify(translate('Resource address copied'))
      } catch { notify(translate('Unable to copy resource address'), 'error') }
      return
    }
    const selectButton = event.target.closest('[data-resource-select]')
    if (selectButton) {
      state.selectedId = state.selectedId === selectButton.dataset.resourceSelect ? '' : selectButton.dataset.resourceSelect
      render()
    }
  }

  function syncLauncher(count) {
    const button = element('open-thread-resources')
    const badge = element('thread-resources-count')
    if (!button || !badge) return
    badge.textContent = count > 99 ? '99+' : String(count)
    badge.classList.toggle('hidden', count < 1)
    button.setAttribute('aria-pressed', String(isOpen()))
    const title = count ? `${translate('Resources')} · ${count}` : translate('Resources')
    button.title = title
    button.setAttribute('aria-label', title)
  }

  function knownCount(state) {
    return state?.built ? state.index.counts().all : 0
  }

  function sourceChanged(state) {
    const model = currentModel()
    return !state?.built
      || state.builtModel !== model
      || state.builtLatestTurn !== latestTurn(model)
  }

  function invalidate(backend, threadId) {
    const key = threadId ? `${backend || 'codex'}:${threadId}` : ''
    const state = key && states.get(key)
    if (!state) return
    state.dirty = true
    if (currentKey() !== key) return
    if (isOpen()) sync()
    else {
      syncLauncher(knownCount(state))
      scheduleLazyRebuild(state)
    }
  }

  function cancelLazyScan() {
    pendingLazyScan?.cancel?.()
    pendingLazyScan = null
  }

  function scheduleLazyRebuild(state) {
    cancelLazyScan()
    const key = state?.key
    if (!key) return
    const job = { key, cancel: null }
    job.cancel = scheduleIdle(() => {
      if (pendingLazyScan !== job) return
      pendingLazyScan = null
      if (isOpen() || currentKey() !== key || stateForCurrent() !== state) return
      const revision = currentRevision()
      if (!state.built || state.builtRevision !== revision) rebuild({ revision })
      else {
        state.dirty = false
        syncLauncher(knownCount(state))
      }
    })
    pendingLazyScan = job
  }

  function openForDebug(root) {
    const path = String(root || '')
    developerContext = {
      backend: 'codex',
      thread: { id: 'developer-resources-preview', cwd: path },
      model: {
        turns: [{
          id: 'preview-1',
          items: [
            { id: 'preview-user', type: 'userMessage', content: 'Please review src-tauri/src/main.rs:640 and /tmp/external-result.json.' },
            { id: 'preview-agent', type: 'agentMessage', text: `Design references: https://code.visualstudio.com/api/references/vscode-api and \`docs/session-resources/specification.md\`.` },
            { id: 'preview-change', type: 'fileChange', changes: [{ kind: 'update', path: 'ui/session-resources.mjs' }] },
          ],
        }],
      },
    }
    open()
  }

  function currentRevision() {
    const thread = currentThread()
    if (!thread) return 'none'
    return `${currentBackend()}:${thread.id}:${thread.cwd || ''}:${sessionResourceRevision(currentModel())}`
  }

  return { bind, sync, syncSelection, invalidate, rebuild, open, openForDebug, close, isOpen, render, currentIndex: () => stateForCurrent()?.index || null }
}

function latestTurn(model) {
  return Array.isArray(model?.turns) ? model.turns.at(-1) || null : null
}

function scheduleLazyResourceScan(callback) {
  let cancelled = false
  let frameHandle = null
  let paintTimer = null
  let idleHandle = null
  let idleTimer = null

  const run = () => {
    if (!cancelled) callback()
  }
  const afterPaint = () => {
    frameHandle = null
    paintTimer = null
    if (cancelled) return
    if (typeof globalThis.requestIdleCallback === 'function') {
      idleHandle = globalThis.requestIdleCallback(run, { timeout: 2_000 })
    } else {
      idleTimer = setTimeout(run, 0)
    }
  }

  if (typeof globalThis.requestAnimationFrame === 'function') {
    frameHandle = globalThis.requestAnimationFrame(afterPaint)
  } else {
    paintTimer = setTimeout(afterPaint, 0)
  }

  return () => {
    cancelled = true
    if (frameHandle != null && typeof globalThis.cancelAnimationFrame === 'function') {
      globalThis.cancelAnimationFrame(frameHandle)
    }
    if (paintTimer != null) clearTimeout(paintTimer)
    if (idleHandle != null && typeof globalThis.cancelIdleCallback === 'function') {
      globalThis.cancelIdleCallback(idleHandle)
    }
    if (idleTimer != null) clearTimeout(idleTimer)
  }
}

function favoriteSvg() {
  return '<svg viewBox="0 0 18 18" aria-hidden="true"><path d="m9 2.8 2.02 4.09 4.51.66-3.27 3.18.77 4.5L9 13.11l-4.03 2.12.77-4.5-3.27-3.18 4.51-.66Z"></path></svg>'
}

function openLabel(resource, translate) {
  const group = resourceGroup(resource.kind)
  if (group === 'web') return translate('Open in Browser')
  if (group === 'code') return translate('Go to code')
  return resource.kind === 'directory' ? translate('Reveal in Files') : translate('Open file')
}

function sourceLabel(occurrence, index, translate) {
  const labels = {
    userMessage: translate('User message'),
    agentMessage: translate('Agent response'),
    commandExecution: translate('Command output'),
    fileChange: translate('File changes'),
    webSearch: translate('Web search'),
  }
  return `${labels[occurrence.itemType] || occurrence.itemType || translate('Message')} · ${translate('Reference')} ${index + 1}`
}

function resourceKindSvg(kind) {
  if (resourceGroup(kind) === 'web') return '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M4 12h16M12 4a12 12 0 0 1 0 16M12 4a12 12 0 0 0 0 16"/></svg>'
  if (kind === 'code') return '<svg viewBox="0 0 24 24"><path d="m9 7-5 5 5 5M15 7l5 5-5 5M13 5l-2 14"/></svg>'
  if (kind === 'directory') return '<svg viewBox="0 0 24 24"><path d="M3.5 6.5h6l2 2h9v10h-17zM3.5 9h17"/></svg>'
  return '<svg viewBox="0 0 24 24"><path d="M6 3.5h8l4 4v13H6zM14 3.5v4h4M9 12h6M9 15.5h6"/></svg>'
}

function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  })[character])
}
