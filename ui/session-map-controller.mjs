import { backendDescriptor, isCodexBackend } from './backends.mjs'
import { questionForTurn } from './favorites.mjs'
import { t } from './i18n.mjs'
import {
  assistantOperationSchema,
  bootstrapMapInput,
  flattenSessionMap,
  mapItemTrail,
  mapProgress,
  normalizeSessionMap,
  parseSessionMapUpdate,
  safeAssistantOperations,
  SESSION_MAP_UPDATE_START,
  SessionMapWorkerPool,
  sessionMapEndpoint,
  sessionMapKey,
  sessionMapTurnConfiguration,
  sessionMapVisibleText,
  shouldBootstrapSessionMap,
  structuredWorkerText,
  visibleMapItems,
} from './session-map.mjs'
import { sessionRefKey } from './thread-router.mjs'

export function createSessionMapRuntimeState() {
  return {
    sessionMaps: new Map(),
    sessionMapLoads: new Map(),
    sessionMapDismissed: new Set(),
    sessionMapSelectedItem: null,
    sessionMapMenuItem: null,
    sessionMapSync: new Map(),
    sessionMapWorkerRequests: new Map(),
    sessionMapWorkers: new SessionMapWorkerPool(),
    sessionMapBootstrapAttempts: new Set(),
    sessionMapInlineProcessing: new Set(),
  }
}

export function createSessionMapController({
  state,
  transport,
  model,
  view,
  randomId,
  notify = () => {},
  reportError = console.error,
}) {
  const { gatewayFetch, gatewayWebSocket, rpc, dispatchBackendRpc, sendRaw } = transport
  const { createViewModel: createCodexViewModel, applyNotification: applyCodexNotification } = model
  const {
    selectedStateKey,
    activateRightWorkspace,
    closeActionMenus,
    toggleActionMenu,
    closeAnnotationRail,
    closeFavoritesRail,
    renderArtifact,
    isResourcesOpen,
    isWorkspaceOpen,
  } = view
  const $ = (selector) => document.querySelector(selector)
  const sessionResources = { isOpen: isResourcesOpen }
  const workspaceTools = { isOpen: isWorkspaceOpen }
  const toast = notify
  const showError = reportError
  const routerRuntimeKey = sessionRefKey
  let sessionMapRequestId = -8_500_000

  function bind() {
    $('#session-map-action')?.addEventListener('click', handleSessionMapAction)
    $('#session-map-more')?.addEventListener('click', () => toggleActionMenu('session-map-menu', 'session-map-more'))
    $('#close-session-map')?.addEventListener('click', closeSessionMapRail)
    $('#session-map-add-root')?.addEventListener('click', () => openSessionMapItemDialog())
    $('#session-map-ai-generate')?.addEventListener('click', () => generateSessionMapStructure().catch(showError))
    $('#session-map-edit-goal')?.addEventListener('click', openSessionMapGoalDialog)
    $('#session-map-undo')?.addEventListener('click', () => undoSessionMap().catch(showError))
    $('#session-map-delete')?.addEventListener('click', () => deleteSessionMap().catch(showError))
    $('#session-map-tree')?.addEventListener('click', handleSessionMapTreeClick)
    $('#session-map-item-menu')?.addEventListener('click', handleSessionMapItemMenu)
    $('#session-map-form')?.addEventListener('submit', createSessionMap)
    $('#close-session-map-dialog')?.addEventListener('click', closeSessionMapDialog)
    $('#cancel-session-map')?.addEventListener('click', closeSessionMapDialog)
    $('#session-map-goal-form')?.addEventListener('submit', saveSessionMapGoal)
    $('#session-map-suggest-goal')?.addEventListener('click', () => suggestSessionMapGoal().catch(showError))
    $('#close-session-map-goal')?.addEventListener('click', () => $('#session-map-goal-dialog').close())
    $('#cancel-session-map-goal')?.addEventListener('click', () => $('#session-map-goal-dialog').close())
    $('#session-map-item-form')?.addEventListener('submit', saveSessionMapItem)
    $('#close-session-map-item')?.addEventListener('click', () => $('#session-map-item-dialog').close())
    $('#cancel-session-map-item')?.addEventListener('click', () => $('#session-map-item-dialog').close())
  }

  function captureSessionMapWorkerResponse(message) {
    const id = String(message?.id ?? '')
    const method = state.sessionMapWorkerRequests.get(id)
    if (!method) return
    state.sessionMapWorkerRequests.delete(id)
    if (message.error) return
    if (method === 'thread/start' && message.result?.thread?.id) {
      state.hiddenCodexThreads.add(sessionRefKey(state.backend, message.result.thread.id))
    }
    if (method === 'turn/start' && message.result?.turn?.id) {
      state.hiddenCodexTurns.add(routerRuntimeKey(state.backend, message.result.turn.id))
    }
  }

  async function handleSessionMapToolCall(message) {
    const backend = state.backend
    const params = message.params || {}
    const key = sessionMapKey(backend, params.threadId)
    try {
      const map = await loadSessionMap(backend, params.threadId)
      if (!map) throw new Error('This thread does not have a Session Map')
      const operations = safeAssistantOperations(params.arguments)
      if (!operations.length) throw new Error('No safe Session Map operations were provided')
      const updated = await applySessionMapOperations(operations, {
        actor: 'assistant',
        sourceTurnId: params.turnId || null,
        key,
      })
      sendRaw({
        id: message.id,
        result: {
          success: true,
          contentItems: [{ type: 'inputText', text: `Session Map updated to revision ${updated.revision}.` }],
        },
      })
      state.sessionMapSync.set(key, { state: 'synced', message: 'Map updated during the current turn' })
      if (selectedStateKey() === key) renderSessionMap()
    } catch (error) {
      sendRaw({
        id: message.id,
        result: {
          success: false,
          contentItems: [{ type: 'inputText', text: `Session Map update rejected: ${error.message}` }],
        },
      })
      state.sessionMapSync.set(key, { state: 'error', message: error.message })
      if (selectedStateKey() === key) renderSessionMap()
    }
  }

  function selectedSessionMap() {
    return state.sessionMaps.get(selectedStateKey()) || null
  }

  async function sessionMapFetch(path, { method = 'GET', body } = {}) {
    const response = await gatewayFetch(path, {
      method,
      headers: body == null ? {} : { 'Content-Type': 'application/json' },
      body: body == null ? undefined : JSON.stringify(body),
      cache: 'no-store',
    })
    const value = await response.json().catch(() => null)
    if (!response.ok) {
      const error = new Error(value?.error?.message || `${method} ${path}: HTTP ${response.status}`)
      error.status = response.status
      throw error
    }
    return value
  }

  async function loadSessionMap(backend, threadId, { force = false } = {}) {
    const key = sessionMapKey(backend, threadId)
    if (!key) return null
    if (!force && state.sessionMaps.has(key)) return state.sessionMaps.get(key)
    if (state.sessionMapLoads.has(key)) return state.sessionMapLoads.get(key)
    const load = sessionMapFetch(sessionMapEndpoint(backend, threadId))
      .then((value) => normalizeSessionMap(value))
      .catch((error) => {
        if (error.status === 404) return null
        throw error
      })
      .then((map) => {
        state.sessionMaps.set(key, map)
        if (selectedStateKey() === key) renderSessionMap()
        return map
      })
      .finally(() => state.sessionMapLoads.delete(key))
    state.sessionMapLoads.set(key, load)
    return load
  }

  function handleSessionMapAction() {
    closeActionMenus()
    if (!state.selectedId) return
    if (selectedSessionMap()) openSessionMapRail()
    else openSessionMapDialog()
  }

  function openSessionMapDialog() {
    if (!state.selectedId) return
    closeActionMenus()
    const opening = state.openingMessages[selectedStateKey()]?.text || ''
    $('#session-map-create-goal').value = opening.length <= 500 ? opening : ''
    $('#session-map-create-done').value = ''
    $('#session-map-structure').value = 'hierarchy'
    $('#session-map-create-error').classList.add('hidden')
    $('#session-map-dialog').showModal()
    $('#session-map-create-goal').focus()
  }

  function closeSessionMapDialog() {
    $('#session-map-dialog').close()
  }

  async function createSessionMap(event) {
    event.preventDefault()
    const key = selectedStateKey()
    if (!key || !state.selectedId) return
    const payload = {
      backend: state.backend,
      threadId: state.selectedId,
      goal: $('#session-map-create-goal').value.trim(),
      definitionOfDone: $('#session-map-create-done').value.trim(),
      structure: $('#session-map-structure').value,
      items: [],
    }
    const errorElement = $('#session-map-create-error')
    errorElement.classList.add('hidden')
    try {
      const map = normalizeSessionMap(await sessionMapFetch('/studio/session-map', { method: 'POST', body: payload }))
      state.sessionMaps.set(key, map)
      state.sessionMapDismissed.delete(key)
      closeSessionMapDialog()
      closeAnnotationRail()
      closeFavoritesRail()
      renderSessionMap()
      toast('Map created')
      generateSessionMapStructure({ key, model: state.model, automatic: true }).catch((error) => {
        console.warn('Unable to generate initial Session Map structure', error)
      })
    } catch (error) {
      errorElement.textContent = error.message
      errorElement.classList.remove('hidden')
    }
  }

  function openSessionMapRail() {
    const key = selectedStateKey()
    if (!key || !selectedSessionMap()) return
    state.sessionMapDismissed.delete(key)
    activateRightWorkspace('map')
    renderSessionMap()
  }

  function closeSessionMapRail() {
    const key = selectedStateKey()
    if (key) state.sessionMapDismissed.add(key)
    $('#session-map-rail').classList.add('hidden')
    closeSessionMapItemMenu()
    if (state.artifact) renderArtifact()
  }

  function renderSessionMap() {
    const rail = $('#session-map-rail')
    const key = selectedStateKey()
    const map = key ? state.sessionMaps.get(key) : null
    const hasMap = Boolean(map)
    $('#session-map-action span').textContent = t(hasMap ? 'Open Map' : 'Create Map')
    const anotherDockIsOpen = (state.artifact && !$('#artifact-rail').classList.contains('hidden'))
      || !$('#annotation-rail').classList.contains('hidden')
      || !$('#favorites-rail').classList.contains('hidden')
      || sessionResources.isOpen()
      || workspaceTools.isOpen()
    if (!hasMap || state.sessionMapDismissed.has(key) || anotherDockIsOpen) {
      rail.classList.add('hidden')
      return
    }
    rail.classList.remove('hidden')
    rail.dataset.structure = map.structure
    $('#session-map-goal').textContent = map.goal
    $('#session-map-definition').textContent = map.definitionOfDone
    $('#session-map-definition').classList.toggle('hidden', !map.definitionOfDone)

    const items = visibleMapItems(map)
    if (!items.some((item) => item.id === state.sessionMapSelectedItem)) {
      state.sessionMapSelectedItem = map.currentItemId || null
    }
    const trail = mapItemTrail(map, state.sessionMapSelectedItem || map.currentItemId)
    const trailElement = $('#session-map-trail')
    trailElement.innerHTML = trail.map((item, index) => `${index ? '<b>›</b>' : ''}<span>${escapeHtml(item.title)}</span>`).join('')
    trailElement.classList.toggle('hidden', trail.length < 2)

    const tree = $('#session-map-tree')
    const emptySync = state.sessionMapSync.get(key)
    const emptyDescription = emptySync?.state === 'syncing'
      ? t('AI is generating the initial structure…')
      : isCodexBackend(map.backend)
        ? t('There are no items yet. Generate them with AI or add one manually.')
        : t('There are no items yet. Add the first one manually.')
    tree.innerHTML = items.length
      ? flattenSessionMap(map).map(({ item, depth }) => renderSessionMapRow(item, depth, map)).join('')
      : `<div class="session-map-empty"><span>⌁</span><strong>${t('This Map is empty')}</strong><p>${emptyDescription}</p><div class="session-map-empty-actions">${isCodexBackend(map.backend) ? `<button class="subtle-button" type="button" data-map-empty-ai${emptySync?.state === 'syncing' ? ' disabled' : ''}>${t('Generate with AI')}</button>` : ''}<button class="subtle-button" type="button" data-map-empty-add>${t('Add')}</button></div></div>`

    const progress = mapProgress(map)
    $('#session-map-progress').textContent = t('{explored}/{total} visited · {done} done', progress)
    $('#session-map-revision').textContent = `rev ${map.revision}`
    $('#session-map-ai-generate').textContent = items.length ? t('Complete with AI') : t('Generate with AI')
    $('#session-map-ai-generate').disabled = emptySync?.state === 'syncing'
    const sync = state.sessionMapSync.get(key) || (isCodexBackend(map.backend)
      ? { state: 'synced', message: 'Waiting for the next interaction' }
      : { state: '', message: 'OpenCode Maps are currently maintained manually' })
    setSessionMapSyncState(sync.state, sync.message)
  }

  function renderSessionMapRow(item, depth, map) {
    const selected = state.sessionMapSelectedItem === item.id
    const current = map.currentItemId === item.id
    const description = item.summary ? `<small data-no-i18n>${escapeHtml(item.summary)}</small>` : ''
    return `<div class="session-map-row${current ? ' current' : ''}${selected ? ' selected' : ''}" style="--map-depth:${Math.min(depth, 12)}" data-map-item-id="${escapeHtml(item.id)}">
      <span class="session-map-state ${escapeHtml(item.state)}" title="${escapeHtml(mapStateLabel(item.state))}" aria-label="${escapeHtml(mapStateLabel(item.state))}"></span>
      <button class="session-map-row-main" type="button" data-map-item-select="${escapeHtml(item.id)}">
        <span class="session-map-row-copy"><strong data-no-i18n>${escapeHtml(item.title)}</strong>${description}</span>
      </button>
      <button class="session-map-row-menu" type="button" data-map-item-menu="${escapeHtml(item.id)}" aria-label="Item actions">•••</button>
    </div>`
  }

  function mapStateLabel(value) {
    return ({ notStarted: 'Not started', active: 'Current', visited: 'Visited', done: 'Done', paused: 'Pause' })[value] || value
  }

  function setSessionMapSyncState(value, message = '') {
    const element = $('#session-map-sync-state')
    element.className = `session-map-sync-state ${value || ''}`
    element.title = message || 'Map sync status'
    const key = selectedStateKey()
    if (key) state.sessionMapSync.set(key, { state: value, message })
  }

  function handleSessionMapTreeClick(event) {
    if (event.target.closest('[data-map-empty-ai]')) {
      generateSessionMapStructure().catch(showError)
      return
    }
    if (event.target.closest('[data-map-empty-add]')) {
      openSessionMapItemDialog()
      return
    }
    const menuButton = event.target.closest('[data-map-item-menu]')
    if (menuButton) {
      openSessionMapItemMenu(menuButton.dataset.mapItemMenu, menuButton)
      return
    }
    const selectButton = event.target.closest('[data-map-item-select]')
    if (!selectButton) return
    const itemId = selectButton.dataset.mapItemSelect
    state.sessionMapSelectedItem = itemId
    applySessionMapOperations([{ op: 'setCurrent', itemId }], { actor: 'user' }).catch(showError)
  }

  function openSessionMapItemMenu(itemId, anchor) {
    const menu = $('#session-map-item-menu')
    state.sessionMapMenuItem = itemId
    const bounds = anchor.getBoundingClientRect()
    menu.style.left = `${Math.max(8, Math.min(bounds.right - 145, window.innerWidth - 153))}px`
    menu.style.top = `${Math.max(8, Math.min(bounds.bottom + 5, window.innerHeight - 305))}px`
    menu.classList.remove('hidden')
  }

  function closeSessionMapItemMenu() {
    $('#session-map-item-menu')?.classList.add('hidden')
    state.sessionMapMenuItem = null
  }

  async function handleSessionMapItemMenu(event) {
    const action = event.target.closest('[data-map-item-action]')?.dataset.mapItemAction
    const itemId = state.sessionMapMenuItem
    if (!action || !itemId) return
    closeSessionMapItemMenu()
    const map = selectedSessionMap()
    const item = map?.items.find((candidate) => candidate.id === itemId)
    if (!item) return
    if (action === 'add-child') return openSessionMapItemDialog(itemId)
    if (action === 'edit') return openSessionMapItemDialog(item.parentId, item)
    if (action === 'archive') {
      if (!window.confirm(t('Remove “{title}” and its children? You can undo immediately.', { title: item.title }))) return
      return applySessionMapOperations([{ op: 'archiveItem', itemId }], { actor: 'user' }).catch(showError)
    }
    if (action === 'current') {
      state.sessionMapSelectedItem = itemId
      return applySessionMapOperations([{ op: 'setCurrent', itemId }], { actor: 'user' }).catch(showError)
    }
    return applySessionMapOperations([{ op: 'setState', itemId, state: action }], { actor: 'user' }).catch(showError)
  }

  function openSessionMapItemDialog(parentId = null, item = null) {
    const map = selectedSessionMap()
    if (!map) return
    closeActionMenus()
    closeSessionMapItemMenu()
    $('#session-map-item-dialog-title').textContent = item ? t('Edit item') : t('Add item')
    $('#session-map-item-id').value = item?.id || ''
    $('#session-map-item-title').value = item?.title || ''
    $('#session-map-item-kind').value = item?.kind || 'item'
    $('#session-map-item-summary').value = item?.summary || ''
    const parent = $('#session-map-item-parent')
    parent.innerHTML = `<option value="">${t('Top level')}</option>${visibleMapItems(map)
      .filter((candidate) => candidate.id !== item?.id)
      .map((candidate) => `<option value="${escapeHtml(candidate.id)}">${escapeHtml(candidate.title)}</option>`)
      .join('')}`
    parent.value = item?.parentId || parentId || ''
    parent.disabled = Boolean(item)
    $('#session-map-item-error').classList.add('hidden')
    $('#session-map-item-dialog').showModal()
    $('#session-map-item-title').focus()
  }

  async function saveSessionMapItem(event) {
    event.preventDefault()
    const itemId = $('#session-map-item-id').value
    const title = $('#session-map-item-title').value.trim()
    const kind = $('#session-map-item-kind').value.trim() || 'item'
    const summary = $('#session-map-item-summary').value.trim()
    const operation = itemId
      ? { op: 'updateItem', itemId, title, kind, summary }
      : {
          op: 'addItem', itemId: randomId(), parentId: $('#session-map-item-parent').value || null,
          afterItemId: null, title, kind, summary, state: 'notStarted',
        }
    const errorElement = $('#session-map-item-error')
    errorElement.classList.add('hidden')
    try {
      await applySessionMapOperations([operation], { actor: 'user' })
      $('#session-map-item-dialog').close()
    } catch (error) {
      errorElement.textContent = error.message
      errorElement.classList.remove('hidden')
    }
  }

  function openSessionMapGoalDialog() {
    const map = selectedSessionMap()
    if (!map) return
    closeActionMenus()
    $('#session-map-goal-input').value = map.goal
    $('#session-map-done-input').value = map.definitionOfDone
    $('#session-map-goal-error').classList.add('hidden')
    $('#session-map-goal-dialog').showModal()
    $('#session-map-goal-input').focus()
  }

  async function suggestSessionMapGoal() {
    const map = selectedSessionMap()
    if (!map) return
    if (!isCodexBackend(state.backend)) throw new Error('OpenCode sessions do not support AI goal regeneration yet')
    const button = $('#session-map-suggest-goal')
    const original = button.textContent
    button.disabled = true
    button.textContent = t('Generating…')
    const recent = (state.model.turns || []).slice(-6).map((turn, index) => ({
      turn: index + 1,
      user: questionForTurn(turn).slice(0, 8_000),
      assistant: answerForMapTurn(turn).slice(0, 12_000),
    }))
    try {
      const result = await runCodexStructuredWorker({
        key: selectedStateKey(),
        developerInstructions: 'Infer a concise navigation goal for an existing conversation. Do not use tools or answer the user. Return only the JSON object required by the output schema. The result is a suggestion that the user will review; do not modify any state.',
        input: `Current goal: ${map.goal}\nCurrent completion definition: ${map.definitionOfDone}\nRecent interactions: ${JSON.stringify(recent)}\nSuggest one concise goal and an observable completion definition that match the conversation's present direction.`,
        outputSchema: {
          type: 'object',
          properties: {
            goal: { type: 'string', minLength: 1, maxLength: 500 },
            definitionOfDone: { type: 'string', maxLength: 1000 },
          },
          required: ['goal', 'definitionOfDone'],
          additionalProperties: false,
        },
        timeoutMessage: 'AI goal generation timed out',
      })
      if (!result?.goal) throw new Error('AI did not return a usable goal')
      $('#session-map-goal-input').value = result.goal
      $('#session-map-done-input').value = result.definitionOfDone || ''
      toast('AI suggestion filled in; review it before saving')
    } finally {
      button.disabled = false
      button.textContent = original
    }
  }

  async function saveSessionMapGoal(event) {
    event.preventDefault()
    const errorElement = $('#session-map-goal-error')
    errorElement.classList.add('hidden')
    try {
      await applySessionMapOperations([{
        op: 'setGoal',
        goal: $('#session-map-goal-input').value.trim(),
        definitionOfDone: $('#session-map-done-input').value.trim(),
      }], { actor: 'user' })
      $('#session-map-goal-dialog').close()
    } catch (error) {
      errorElement.textContent = error.message
      errorElement.classList.remove('hidden')
    }
  }

  async function applySessionMapOperations(operations, { actor = 'user', sourceTurnId = null, key = selectedStateKey() } = {}) {
    const [backend, ...threadParts] = key.split(':')
    const threadId = threadParts.join(':')
    const map = state.sessionMaps.get(key)
    if (!map || !backend || !threadId || !operations.length) return map
    try {
      const value = await sessionMapFetch(sessionMapEndpoint(backend, threadId, 'operations'), {
        method: 'POST',
        body: { baseRevision: map.revision, actor, sourceTurnId, operations },
      })
      const updated = normalizeSessionMap(value)
      state.sessionMaps.set(key, updated)
      if (selectedStateKey() === key) renderSessionMap()
      return updated
    } catch (error) {
      if (error.status === 409) await loadSessionMap(backend, threadId, { force: true })
      throw error
    }
  }

  async function undoSessionMap() {
    closeActionMenus()
    const key = selectedStateKey()
    const map = selectedSessionMap()
    if (!key || !map) return
    const value = await sessionMapFetch(sessionMapEndpoint(map.backend, map.threadId, 'undo'), { method: 'POST' })
    state.sessionMaps.set(key, normalizeSessionMap(value))
    renderSessionMap()
    toast('Undid the latest Map update')
  }

  async function deleteSessionMap() {
    closeActionMenus()
    const key = selectedStateKey()
    const map = selectedSessionMap()
    if (!key || !map || !window.confirm(t('Delete this session Map? Conversation history will not be affected.'))) return
    await sessionMapFetch(sessionMapEndpoint(map.backend, map.threadId), { method: 'DELETE' })
    if (isCodexBackend(map.backend) && state.backend === map.backend && state.ready) {
      rpc('thread/resume', {
        threadId: map.threadId,
        developerInstructions: null,
        dynamicTools: [],
      }).catch((error) => console.warn('Unable to clear Session Map thread context', error))
    }
    state.sessionMaps.set(key, null)
    state.sessionMapDismissed.delete(key)
    state.sessionMapSync.delete(key)
    state.sessionMapSelectedItem = null
    disposeSessionMapWorker(key)
    renderSessionMap()
    toast('Map deleted')
  }

  function maybeBootstrapSessionMap(key, model) {
    const map = state.sessionMaps.get(key)
    if (!shouldBootstrapSessionMap(map) || state.sessionMapBootstrapAttempts.has(key)) return
    generateSessionMapStructure({ key, model, automatic: true }).catch((error) => {
      console.warn('Session Map initial generation failed', error)
    })
  }

  async function generateSessionMapStructure({ key = selectedStateKey(), model = state.model, automatic = false } = {}) {
    closeActionMenus()
    const separator = key.indexOf(':')
    const backend = separator > 0 ? key.slice(0, separator) : ''
    const map = state.sessionMaps.get(key)
    if (!map) return null
    if (!isCodexBackend(backend)) {
      if (automatic) return null
      throw new Error('OpenCode sessions do not support AI Map generation yet')
    }
    if (automatic && state.sessionMapBootstrapAttempts.has(key)) return map
    if (automatic) state.sessionMapBootstrapAttempts.add(key)

    const interactions = (model?.turns || []).map((turn) => ({
      user: questionForTurn(turn).trim(),
      assistant: answerForMapTurn(turn),
    })).filter((interaction) => interaction.user || interaction.assistant)
    const sourceTurn = [...(model?.turns || [])].reverse().find((turn) => turn?.id && (questionForTurn(turn).trim() || answerForMapTurn(turn)))
    state.sessionMapSync.set(key, { state: 'syncing', message: 'AI is generating the initial Map' })
    if (selectedStateKey() === key) renderSessionMap()

    try {
      const result = await runCodexStructuredWorker({
        key,
        developerInstructions: 'You create a compact navigation Map for another conversation. Do not use tools, inspect files, or answer the user. Return only the JSON object required by the supplied output schema. Follow the safe-operation restrictions exactly.',
        input: bootstrapMapInput(map, interactions),
        outputSchema: assistantOperationSchema(),
        timeoutMessage: 'AI Map generation timed out',
      })
      const operations = safeAssistantOperations(result)
      if (!operations.length) throw new Error('AI did not generate any usable Map items')
      const updated = await applySessionMapOperations(operations, {
        actor: 'assistant',
        sourceTurnId: sourceTurn?.id ? String(sourceTurn.id) : null,
        key,
      })
      state.sessionMapSync.set(key, { state: 'synced', message: 'Map structure updated' })
      if (selectedStateKey() === key) renderSessionMap()
      return updated
    } catch (error) {
      state.sessionMapSync.set(key, { state: 'error', message: error.message })
      if (selectedStateKey() === key) renderSessionMap()
      throw error
    }
  }

  async function processSessionMapInlineUpdate(backend, threadId, model, completedTurnId = null) {
    if (!isCodexBackend(backend) || !threadId || !model) return
    const key = sessionMapKey(backend, threadId)
    const processingKey = `${key}:${completedTurnId || ''}`
    if (state.sessionMapInlineProcessing.has(processingKey)) return
    state.sessionMapInlineProcessing.add(processingKey)
    try {
      const map = await loadSessionMap(backend, threadId)
      if (!map) return
      const turn = completedTurnId
        ? (model.turns || []).find((candidate) => String(candidate.id) === String(completedTurnId))
        : [...(model.turns || [])].reverse().find((candidate) => candidate?.id)
      if (!turn?.id || String(turn.id) === map.lastSyncedTurnId) return
      const message = [...(turn.items || [])].reverse().find((item) =>
        item?.type === 'agentMessage' && String(item.text || '').includes(SESSION_MAP_UPDATE_START),
      )
      if (!message) throw new Error(t('This response did not include a Map update block. You can run AI Complete manually.'))
      const parsed = parseSessionMapUpdate(message.text)
      if (!parsed.found) throw new Error(t('This response did not include a Map update block'))
      if (parsed.update.baseRevision !== map.revision) {
        throw new Error(t('The Map update is stale: the response used rev {responseRevision}, but the current revision is {currentRevision}', {
          responseRevision: parsed.update.baseRevision,
          currentRevision: map.revision,
        }))
      }
      if (parsed.update.operations.length > 40) throw new Error(t('The Map update exceeds the 40-operation limit'))
      const operations = safeAssistantOperations(parsed.update)
      if (operations.length !== parsed.update.operations.length) throw new Error(t('The Map update contains an unsafe or unknown operation'))
      if (operations.length) {
        await applySessionMapOperations(operations, {
          actor: 'assistant',
          sourceTurnId: String(turn.id),
          key,
        })
      } else {
        map.lastSyncedTurnId = String(turn.id)
      }
      state.sessionMapSync.set(key, {
        state: 'synced',
        message: operations.length ? t('Map updated from this response') : t('This response did not require a Map change'),
      })
      if (selectedStateKey() === key) renderSessionMap()
    } catch (error) {
      state.sessionMapSync.set(key, { state: 'error', message: t(error.message) })
      if (selectedStateKey() === key) renderSessionMap()
      throw error
    } finally {
      state.sessionMapInlineProcessing.delete(processingKey)
    }
  }

  function answerForMapTurn(turn) {
    return (turn?.items || [])
      .filter((item) => item?.type === 'agentMessage' && item.text)
      .map((item) => sessionMapVisibleText(item.text).trim())
      .filter(Boolean)
      .join('\n\n')
  }

  function runCodexStructuredWorker({ key, developerInstructions, input, outputSchema, timeoutMessage }) {
    return state.sessionMapWorkers.enqueue(key, (worker) =>
      runCodexStructuredWorkerTurn(worker, { developerInstructions, input, outputSchema, timeoutMessage }),
    )
  }

  function disposeSessionMapWorker(key) {
    const worker = state.sessionMapWorkers.dispose(key)
    if (!worker) return
    worker.chain.finally(() => {
      const backend = worker.key.split(':', 1)[0]
      if (!worker.threadId || !isCodexBackend(backend)) return
      dispatchBackendRpc(backend, 'thread/delete', { threadId: worker.threadId }).catch((error) => {
        console.warn('Unable to release ephemeral Session Map worker', error)
      })
    })
  }

  function runCodexStructuredWorkerTurn(worker, { developerInstructions, input, outputSchema, timeoutMessage }) {
    return new Promise((resolve, reject) => {
      const backend = worker.key.split(':', 1)[0]
      const descriptor = backendDescriptor(backend)
      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
      const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
      const pending = new Map()
      const buffered = []
      const hiddenModel = createCodexViewModel()
      let hiddenThreadId = worker.threadId
      let hiddenTurnId = null
      let serverGeneration = null
      let started = false
      let settled = false
      const timeout = setTimeout(() => finish(new Error(timeoutMessage || 'Codex structured task timed out')), 150_000)

      const finish = (error, value) => {
        if (settled) return
        settled = true
        clearTimeout(timeout)
        for (const [id, request] of pending) {
          state.sessionMapWorkerRequests.delete(id)
          request.reject(error || new Error('Map sync connection closed'))
        }
        pending.clear()
        socket.onclose = null
        socket.close()
        if (error) reject(error)
        else resolve(value)
      }

      const request = (method, params) => new Promise((requestResolve, requestReject) => {
        const id = --sessionMapRequestId
        pending.set(String(id), { resolve: requestResolve, reject: requestReject })
        state.sessionMapWorkerRequests.set(String(id), method)
        socket.send(JSON.stringify({ id, method, params }))
      })

      const processTurnMessage = (message) => {
        const messageTurnId = message.params?.turnId || message.params?.turn?.id
        if (!hiddenTurnId || String(messageTurnId || '') !== String(hiddenTurnId)) return
        applyCodexNotification(hiddenModel, message)
        if (message.method !== 'turn/completed') return
        const completedTurn = hiddenModel.turns.find((turn) => String(turn.id) === String(hiddenTurnId)) || message.params?.turn
        try {
          finish(null, parseStructuredJson(structuredWorkerText(completedTurn)))
        } catch (error) {
          finish(error)
        }
      }

      const begin = async () => {
        if (started || settled) return
        started = true
        try {
          if (!hiddenThreadId) {
            const thread = await request('thread/start', {
              ephemeral: true,
              approvalPolicy: 'never',
              sandbox: 'read-only',
              developerInstructions: 'You are the single reusable structured worker for one Session Map. Each turn contains authoritative task-specific instructions and state. Do not use tools, inspect files, or answer the end user. Do not rely on earlier worker turns when they conflict with the current input. Return only the JSON required by the current output schema.',
            })
            hiddenThreadId = thread?.thread?.id
            if (!hiddenThreadId) throw new Error('Codex did not create the reusable Map worker')
            worker.threadId = String(hiddenThreadId)
            worker.generation = serverGeneration
          }
          state.hiddenCodexThreads.add(sessionRefKey(backend, hiddenThreadId))
          hiddenModel.threadId = hiddenThreadId
          const result = await request('turn/start', {
            threadId: hiddenThreadId,
            input: [{ type: 'text', text: `Task-specific instructions:\n${developerInstructions}\n\nAuthoritative task input:\n${input}` }],
            outputSchema,
          })
          hiddenTurnId = result?.turn?.id
          if (!hiddenTurnId) throw new Error('Codex did not start the Map reconciliation turn')
          state.hiddenCodexTurns.add(routerRuntimeKey(backend, hiddenTurnId))
          for (const message of buffered.splice(0)) processTurnMessage(message)
        } catch (error) {
          finish(error)
        }
      }

      socket.onmessage = (event) => {
        let message
        try { message = JSON.parse(event.data) } catch { return }
        if (message.method === 'studio/appServer/status') {
          if (message.params?.state === 'ready') {
            serverGeneration = Number(message.params?.generation || 0)
            const staleThreadId = state.sessionMapWorkers.reconcileGeneration(worker, serverGeneration)
            if (staleThreadId) {
              state.hiddenCodexThreads.delete(sessionRefKey(backend, staleThreadId))
              hiddenThreadId = null
            }
            begin()
          }
          else if (message.params?.state === 'error') finish(new Error(message.params?.message || 'Codex App Server unavailable'))
          return
        }
        if (message.id != null && !message.method) {
          captureSessionMapWorkerResponse(message)
          const requestState = pending.get(String(message.id))
          if (!requestState) return
          pending.delete(String(message.id))
          if (message.error) requestState.reject(new Error(message.error.message || JSON.stringify(message.error)))
          else requestState.resolve(message.result)
          return
        }
        if (!message.method || message.method.startsWith('studio/appServer/')) return
        if (!hiddenTurnId) buffered.push(message)
        else processTurnMessage(message)
      }
      socket.onerror = () => finish(new Error(t('Unable to connect to the {backend} Map synchronization service', { backend: descriptor.name })))
      socket.onclose = () => finish(new Error(t('The {backend} Map synchronization connection closed', { backend: descriptor.name })))
    })
  }

  function parseStructuredJson(value) {
    const text = String(value || '').trim()
    if (!text) throw new Error('The structured AI task returned no result')
    const unwrapped = text.startsWith('```')
      ? text.replace(/^```(?:json)?\s*/u, '').replace(/\s*```$/u, '')
      : text
    return JSON.parse(unwrapped)
  }

  async function prepareTurn() {
    if (!isCodexBackend(state.backend) || !state.selectedId) return
    const map = await loadSessionMap(state.backend, state.selectedId)
    if (!map) return
    const configuration = sessionMapTurnConfiguration(map)
    try {
      await rpc('thread/resume', {
        threadId: state.selectedId,
        ...configuration,
      })
    } catch (error) {
      console.debug('Dynamic Session Map tools are unavailable; using developer context only', error)
      await rpc('thread/resume', {
        threadId: state.selectedId,
        developerInstructions: configuration.developerInstructions,
      })
    }
    setSessionMapSyncState('syncing', 'The current Map was added to this turn context')
  }

  function resetSelection() {
    state.sessionMapSelectedItem = null
    closeSessionMapItemMenu()
  }

  function escapeHtml(value) {
    return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
    })[character])
  }

  return {
    applyOperations: applySessionMapOperations,
    bind,
    captureWorkerResponse: captureSessionMapWorkerResponse,
    closeItemMenu: closeSessionMapItemMenu,
    handleToolCall: handleSessionMapToolCall,
    load: loadSessionMap,
    maybeBootstrap: maybeBootstrapSessionMap,
    prepareTurn,
    processInlineUpdate: processSessionMapInlineUpdate,
    render: renderSessionMap,
    resetSelection,
    setSyncState: setSessionMapSyncState,
  }
}
