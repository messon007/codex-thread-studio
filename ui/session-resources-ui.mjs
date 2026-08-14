import { buildSessionResourceIndex, resourceGroup, resourceIcon } from './session-resources.mjs'

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
}) {
  const states = new Map()
  let developerContext = null
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
        index: buildSessionResourceIndex(null),
        query: '',
        filter: 'all',
        selectedId: '',
        built: false,
      })
    }
    return states.get(key)
  }

  function bind() {
    element('open-thread-resources')?.addEventListener('click', open)
    element('close-resources')?.addEventListener('click', close)
    element('refresh-resources')?.addEventListener('click', () => {
      rebuild()
      notify(translate('资源索引已刷新'))
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

  function sync({ rebuild: shouldRebuild = false } = {}) {
    const state = stateForCurrent()
    const button = element('open-thread-resources')
    if (button) button.disabled = !state
    if (!state) {
      close()
      syncLauncher(0)
      return
    }
    if (shouldRebuild || !state.built) rebuild()
    else {
      syncLauncher(state.index.counts().all)
      if (isOpen()) render()
    }
  }

  function rebuild() {
    const state = stateForCurrent()
    const thread = currentThread()
    if (!state || !thread) return null
    state.index = buildSessionResourceIndex(currentModel(), {
      backend: currentBackend(),
      threadId: thread.id,
      root: thread.cwd || '',
    })
    state.built = true
    if (state.selectedId && !state.index.resourcesById.has(state.selectedId)) state.selectedId = ''
    syncLauncher(state.index.counts().all)
    if (isOpen()) render()
    return state.index
  }

  function open() {
    if (!stateForCurrent()) return
    activate?.('resources')
    element('resources-rail')?.classList.remove('hidden')
    rebuild()
    setTimeout(() => element('resources-search')?.focus(), 30)
  }

  function close() {
    element('resources-rail')?.classList.add('hidden')
    deactivate?.('resources')
    developerContext = null
    syncLauncher(stateForCurrent()?.index.counts().all || 0)
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
    element('resources-summary').textContent = translate('{resources} 个资源 · {occurrences} 次引用')
      .replace('{resources}', String(counts.all)).replace('{occurrences}', String(counts.occurrences))
    element('resources-local-state').textContent = translate('仅索引最后一个 Turn')
  }

  function renderResourceCard(resource, state) {
    const occurrences = state.index.occurrences(resource.id)
    const selected = state.selectedId === resource.id
    const group = resourceGroup(resource.kind)
    const target = resource.target?.url || resource.target?.path || resource.raw
    const location = resource.target?.line ? `${target}:${resource.target.line}${resource.target.column ? `:${resource.target.column}` : ''}` : target
    const label = resource.state === 'blocked' ? translate('已阻止') : group === 'web' ? translate('网页') : group === 'code' ? translate('代码') : translate('文件')
    const occurrenceRows = selected ? `<div class="resource-occurrences">${occurrences.map((occurrence, index) => `<button type="button" data-resource-source="${escapeHtml(occurrence.id)}"><span>${sourceLabel(occurrence, index, translate)}</span><small>${escapeHtml(occurrence.excerpt || '')}</small></button>`).join('')}</div>` : ''
    return `<article class="resource-card${selected ? ' selected' : ''}${resource.state === 'blocked' ? ' blocked' : ''}" data-resource-id="${escapeHtml(resource.id)}">
      <button class="resource-card-main" type="button" data-resource-select="${escapeHtml(resource.id)}">
        <span class="resource-kind-icon ${escapeHtml(resourceIcon(resource.kind))}" aria-hidden="true">${resourceKindSvg(resource.kind)}</span>
        <span class="resource-card-copy"><strong>${escapeHtml(resource.display)}</strong><small>${escapeHtml(location)}</small></span>
        <span class="resource-kind-pill">${escapeHtml(label)} · ${occurrences.length}</span>
      </button>
      ${resource.state === 'blocked' ? `<p class="resource-blocked-reason">${escapeHtml(translate(resource.reason || '安全策略阻止了此资源'))}</p>` : ''}
      <div class="resource-card-actions">
        <button class="resource-open" type="button" data-resource-open="${escapeHtml(resource.id)}" ${resource.state === 'blocked' ? 'disabled' : ''}>${escapeHtml(openLabel(resource, translate))}</button>
        <button type="button" data-resource-source="${escapeHtml(occurrences.at(-1)?.id || '')}">${escapeHtml(translate('返回消息'))}</button>
        <button type="button" data-resource-copy="${escapeHtml(resource.id)}">${escapeHtml(translate('复制地址'))}</button>
        <button class="resource-favorite" type="button" data-resource-favorite="${escapeHtml(resource.id)}" title="${escapeHtml(translate('收藏资源'))}" aria-label="${escapeHtml(translate('收藏资源'))}">${favoriteSvg()}</button>
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
        try { await openResource?.(resource) } catch (error) { notify(error?.message || translate('无法打开资源'), 'error') }
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
        notify(translate('资源地址已复制'))
      } catch { notify(translate('无法复制资源地址'), 'error') }
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
    const title = count ? `${translate('资源')} · ${count}` : translate('资源')
    button.title = title
    button.setAttribute('aria-label', title)
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

  return { bind, sync, rebuild, open, openForDebug, close, isOpen, render, currentIndex: () => stateForCurrent()?.index || null }
}

function favoriteSvg() {
  return '<svg viewBox="0 0 18 18" aria-hidden="true"><path d="m9 2.8 2.02 4.09 4.51.66-3.27 3.18.77 4.5L9 13.11l-4.03 2.12.77-4.5-3.27-3.18 4.51-.66Z"></path></svg>'
}

function openLabel(resource, translate) {
  const group = resourceGroup(resource.kind)
  if (group === 'web') return translate('在浏览器打开')
  if (group === 'code') return translate('定位代码')
  return resource.kind === 'directory' ? translate('在文件中显示') : translate('打开文件')
}

function sourceLabel(occurrence, index, translate) {
  const labels = {
    userMessage: translate('用户消息'),
    agentMessage: translate('Agent 回复'),
    commandExecution: translate('命令输出'),
    fileChange: translate('文件修改'),
    webSearch: translate('网页搜索'),
  }
  return `${labels[occurrence.itemType] || occurrence.itemType || translate('消息')} · ${translate('引用')} ${index + 1}`
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
