import { backendDescriptor, isCodexBackend } from './backends.mjs'
import { t } from './i18n.mjs'
import { isSessionDirectoryHidden } from './thread-catalog.mjs'
import {
  DEFAULT_FALLBACK_CONDITION,
  finalAgentText,
  managedRouterThread,
  normalizeThreadRouter,
  parseRouterDecision,
  parseSessionRefKey,
  recoverManagedRouterCatalog,
  routerApplicationContext,
  routerCandidates,
  routerControllerRef,
  routerDecisionForTurn,
  routerDecisionSchema,
  routerDeveloperInstructions,
  sessionRefKey,
  shouldCreateManagedRouter,
} from './thread-router.mjs'

export function createThreadRouterRuntimeState() {
  return {
    pending: new Map(),
    dispatches: new Map(),
    targetTurns: new Map(),
    monitors: new Map(),
    editor: null,
  }
}

export function routerRuntimeKey(backend, turnId) {
  return sessionRefKey(backend, String(turnId || ''))
}

export function createThreadRouterController({
  state,
  dispatch,
  backend,
  catalog,
  model,
  view,
  persistPreferences,
  notify = () => {},
}) {
  const {
    dispatchRpc,
    refreshCatalogs,
    loadBackendInfo,
    configuredTurnOptions,
  } = backend
  const {
    sidebarCatalogs,
    threadTitle,
    threadStatus,
    mergeThread,
    updateLoadedThreadTimestamp,
  } = catalog
  const {
    ensureSessionModel,
    applyNotification,
    cacheThreadModel,
  } = model
  const {
    closeActionMenus,
    renderThreadList,
    renderWorkspace,
    renderTranscript,
    renderComposerState,
    renderItem,
    selectThread,
  } = view
  const runtime = state.routerRuntime
  const element = (id) => document.getElementById(id)

  function bind() {
    element('router-settings-action')?.addEventListener('click', openDialog)
    element('router-form')?.addEventListener('submit', saveSettings)
    element('close-router-dialog')?.addEventListener('click', closeDialog)
    element('cancel-router')?.addEventListener('click', closeDialog)
    element('router-add-fallback')?.addEventListener('click', addFallback)
  }

  function isThread(threadId = state.selectedId, selectedBackend = state.backend) {
    const controller = routerControllerRef(state.router)
    return Boolean(threadId) && controller?.backend === selectedBackend && controller.id === threadId
  }

  function targetCatalogs() {
    return Object.fromEntries(Object.entries(sidebarCatalogs()).map(([catalogBackend, threads]) => [
      catalogBackend,
      (threads || []).filter((thread) => !isSessionDirectoryHidden(
        thread.cwd,
        state.hiddenSessionDirectories,
        state.sessionDirectoryIgnore,
      )),
    ]))
  }

  function currentCandidates() {
    return routerCandidates(state.router, targetCatalogs(), state.openingMessages)
  }

  async function startTurn(text, attachments = []) {
    if (!isThread() || state.model.activeTurnId) throw new Error(t('The Router is still processing the previous request.'))
    const controller = routerControllerRef(state.router)
    if (!controller || !dispatch.supports(controller.backend)) throw new Error(t('The Router backend is currently unavailable.'))
    await refreshCatalogs()
    const candidates = currentCandidates()
    if (!candidates.length) throw new Error(t('The Router has no available target session. Open Router settings first.'))
    const developerInstructions = routerDeveloperInstructions(candidates)
    const imageInputs = (Array.isArray(attachments) ? attachments : []).filter((item) => item?.type === 'image' || item?.type === 'localImage')
    const controllerInput = [...(text ? [{ type: 'text', text }] : []), ...imageInputs]
    const result = await dispatch.startTurn(controller, controllerInput, {
      ...(isCodexBackend(controller.backend)
        ? { additionalContext: routerApplicationContext(candidates) }
        : { developerInstructions }),
      outputSchema: routerDecisionSchema(candidates.map((candidate) => candidate.key)),
      turnOptions: configuredTurnOptions(),
    })
    if (!result?.turn) throw new Error(t('The Router could not start a new turn.'))
    const turnId = String(result.turn.id || '')
    const key = routerRuntimeKey(controller.backend, turnId)
    runtime.pending.set(key, {
      candidateKeys: candidates.map((candidate) => candidate.key),
      requestedAt: Date.now(),
      attachments: imageInputs,
    })
    runtime.dispatches.set(key, { status: 'routing' })
    applyNotification(state.model, { method: 'turn/started', params: { threadId: controller.id, turn: result.turn } })
    cacheThreadModel(controller.backend, controller.id, state.model)
    renderTranscript()
    monitorTurn(controller, turnId)
  }

  async function completeTurn({ backend: turnBackend, turnId, model: turnModel, turn: suppliedTurn }) {
    turnId = String(turnId || '')
    if (!turnId) return
    const key = routerRuntimeKey(turnBackend, turnId)
    const routed = runtime.targetTurns.get(key)
    if (routed) {
      const completed = turnModel.turns?.find((turn) => String(turn.id) === turnId) || suppliedTurn
      const existing = runtime.dispatches.get(routed.routerTurnId) || {}
      runtime.dispatches.set(routed.routerTurnId, {
        ...existing,
        status: completed?.status === 'failed' ? 'failed' : 'completed',
        error: completed?.error?.message || '',
      })
      runtime.targetTurns.delete(key)
      if (isThread()) renderTranscript()
      return
    }
    const pending = runtime.pending.get(key)
    if (!pending) return
    runtime.pending.delete(key)
    const turn = turnModel.turns?.find((candidate) => String(candidate.id) === turnId) || suppliedTurn
    let decisionParsed = false
    try {
      const decision = parseRouterDecision(finalAgentText(turn), pending.candidateKeys)
      decisionParsed = true
      if (decision.action === 'clarify') {
        runtime.dispatches.set(key, { status: 'clarify', decision })
        if (isThread()) renderTranscript()
        return
      }
      const targetRef = parseSessionRefKey(decision.targetSessionKey)
      const target = targetRef && state.threadsByBackend[targetRef.backend]?.find((thread) => thread.id === targetRef.id)
      if (!target) throw new Error(t('The target session no longer exists.'))
      if (!dispatch.supports(targetRef.backend)) throw new Error(t('The target session backend is currently unavailable.'))
      runtime.dispatches.set(key, { status: 'dispatching', decision })
      if (isThread()) renderTranscript()
      await dispatch.prepareTurn(targetRef, { alreadyActive: threadStatus(target) !== 'notLoaded' })
      const targetModel = await ensureSessionModel(targetRef)
      if (targetModel.activeTurnId) throw new Error(t('“{title}” is running and cannot accept a new request yet.', { title: threadTitle(target) }))
      const result = await dispatch.startTurn(targetRef, [
        { type: 'text', text: decision.forwardedPrompt },
        ...(pending.attachments || []),
      ])
      if (!result?.turn) throw new Error(t('The target session could not start a new turn.'))
      applyNotification(targetModel, { method: 'turn/started', params: { threadId: targetRef.id, turn: result.turn } })
      cacheThreadModel(targetRef.backend, targetRef.id, targetModel)
      updateLoadedThreadTimestamp(targetRef.backend, targetRef.id)
      runtime.dispatches.set(key, {
        status: 'running', decision, targetTurnId: String(result.turn.id || ''),
      })
      runtime.targetTurns.set(routerRuntimeKey(targetRef.backend, result.turn.id), {
        routerTurnId: key,
        targetSessionKey: targetRef.key,
      })
      renderThreadList()
      if (isThread() || state.model === targetModel) {
        renderWorkspace()
        renderTranscript()
      }
      monitorTurn(targetRef, result.turn.id)
    } catch (error) {
      runtime.dispatches.set(key, { status: 'failed', error: error.message, decisionInvalid: !decisionParsed })
      if (isThread()) renderTranscript()
      notify(t('Routing failed: {message}', { message: error.message }), 'error')
    } finally {
      if (isThread()) renderComposerState()
    }
  }

  function monitorTurn(ref, turnId) {
    const key = routerRuntimeKey(ref.backend, turnId)
    if (!key || runtime.monitors.has(key)) return
    let failures = 0
    const poll = async () => {
      try {
        const turnModel = await ensureSessionModel(ref)
        const turn = turnModel.turns?.find((candidate) => String(candidate.id) === String(turnId))
        const terminal = turn && turn.status !== 'inProgress' && turnModel.status !== 'running'
        if (terminal) {
          runtime.monitors.delete(key)
          await completeTurn({ backend: ref.backend, turnId, model: turnModel, turn })
          return
        }
        failures = 0
      } catch (error) {
        failures += 1
        if (failures >= 5) {
          runtime.monitors.delete(key)
          const routed = runtime.targetTurns.get(key)
          const dispatchKey = routed?.routerTurnId || key
          runtime.dispatches.set(dispatchKey, { ...(runtime.dispatches.get(dispatchKey) || {}), status: 'failed', error: error.message })
          if (isThread()) renderTranscript()
          return
        }
      }
      runtime.monitors.set(key, setTimeout(poll, 1_000))
    }
    runtime.monitors.set(key, setTimeout(poll, 500))
  }

  function renderTurn(turn, index) {
    const items = Array.isArray(turn.items) ? turn.items : []
    const userItems = items.filter((item) => item.type === 'userMessage').map((item) => renderItem(item, turn.id)).join('')
    const key = routerRuntimeKey(state.backend, turn.id)
    const dispatchState = runtime.dispatches.get(key)
    const decision = dispatchState?.decision || routerDecisionForTurn(turn, currentCandidates().map((candidate) => candidate.key))
    let card = ''
    if (decision?.action === 'clarify') {
      card = `<article class="router-card clarify"><header><span class="router-card-mark">?</span><div><strong>${t('Target clarification needed')}</strong><small>${escapeHtml(decision.reason || '')}</small></div></header><p>${escapeHtml(decision.message)}</p></article>`
    } else if (decision?.action === 'dispatch') {
      const targetRef = parseSessionRefKey(decision.targetSessionKey)
      const target = targetRef && state.threadsByBackend[targetRef.backend]?.find((thread) => thread.id === targetRef.id)
      const status = dispatchState?.status || 'routed'
      const labels = {
        dispatching: 'Dispatching', running: 'Target running', completed: 'Target completed', failed: 'Dispatch failed', routed: 'Routed',
      }
      const targetLabel = target ? threadTitle(target) : decision.targetSessionKey
      const footerLabel = status === 'completed' ? t('The target response is complete') : t('Request sent to the target session')
      const linkLabel = status === 'completed' ? t('Open response') : t('Open session')
      card = `<article class="router-card ${escapeHtml(status)}"><header><span class="router-card-mark">→</span><div><strong>${escapeHtml(targetLabel)}</strong><small>${escapeHtml(decision.reason || '')}</small></div><span class="router-card-status">${t(labels[status] || labels.routed)}</span></header>${dispatchState?.error ? `<p class="router-card-error">${escapeHtml(dispatchState.error)}</p>` : ''}<footer><span>${footerLabel}</span><button type="button" data-router-target="${escapeHtml(targetRef?.id || '')}" data-router-backend="${escapeHtml(targetRef?.backend || '')}" data-router-turn="${escapeHtml(dispatchState?.targetTurnId || '')}">${linkLabel}</button></footer></article>`
    } else if (dispatchState?.status === 'failed') {
      card = `<article class="router-card failed"><header><span class="router-card-mark">!</span><div><strong>${t(dispatchState.decisionInvalid ? 'Invalid Router decision' : 'Routing failed')}</strong><small>${escapeHtml(dispatchState.error || '')}</small></div></header>${dispatchState.decisionInvalid ? renderDecisionDebug(turn) : ''}</article>`
    } else if (turn.status === 'inProgress' || runtime.pending.has(key) || ['routing', 'dispatching'].includes(dispatchState?.status)) {
      card = `<article class="router-card routing"><header><span class="router-card-mark pulse-mark">↝</span><div><strong>${t('Selecting a target session')}</strong><small>${t('Router is comparing session responsibilities')}</small></div></header></article>`
    } else {
      card = `<article class="router-card failed"><header><span class="router-card-mark">!</span><div><strong>${t('Invalid Router decision')}</strong><small>${t('The Router did not return a valid target from the candidate list.')}</small></div></header>${renderDecisionDebug(turn)}</article>`
    }
    return `<section class="turn router-turn" data-turn-id="${escapeHtml(turn.id || '')}"><div class="turn-separator">Turn ${index + 1}</div>${userItems}${card}</section>`
  }

  async function openTarget(target) {
    await selectThread(target.dataset.routerTarget, { backend: target.dataset.routerBackend })
    const turnId = target.dataset.routerTurn
    if (turnId) {
      requestAnimationFrame(() => document.querySelector(`[data-turn-id="${CSS.escape(turnId)}"]`)?.scrollIntoView({ block: 'start' }))
    }
  }

  function removeSession(sessionBackend, threadId) {
    const key = sessionRefKey(sessionBackend, threadId)
    if (state.router.controllers[sessionBackend] === threadId) {
      const controllers = { ...state.router.controllers }
      delete controllers[sessionBackend]
      state.router = normalizeThreadRouter({ ...state.router, controllers })
      return true
    }
    if (state.router.fallbacks.some((entry) => entry.sessionKey === key)) {
      state.router = normalizeThreadRouter({
        ...state.router,
        fallbacks: state.router.fallbacks.filter((entry) => entry.sessionKey !== key),
      })
      return true
    }
    return false
  }

  function openDialog() {
    closeActionMenus()
    runtime.editor = { fallbacks: state.router.fallbacks.map((entry) => ({ ...entry })) }
    renderManagedStatus()
    element('router-error').classList.add('hidden')
    renderFallbacks()
    element('router-dialog').showModal()
    document.querySelector('.router-dialog-body').scrollTop = 0
  }

  function closeDialog() {
    element('router-dialog').close()
    runtime.editor = null
  }

  function renderManagedStatus() {
    const controllerBackend = state.router.controllerBackend
    const id = state.router.controllers[controllerBackend]
    const managed = state.threadsByBackend[controllerBackend]?.find((thread) => thread.id === id)
    element('managed-router-status').textContent = managed
      ? t('{backend} · created and continuously reused · {title}', { backend: backendDescriptor(controllerBackend).name, title: threadTitle(managed) })
      : t('{backend} · Studio will create it on first save', { backend: backendDescriptor(controllerBackend).name })
  }

  function captureFallbacks() {
    if (!runtime.editor) return
    runtime.editor.fallbacks = [...document.querySelectorAll('#router-fallbacks .router-fallback-card')].map((row) => ({
      sessionKey: row.querySelector('.router-fallback-session').value,
      condition: row.querySelector('.router-fallback-condition').value.trim() || DEFAULT_FALLBACK_CONDITION,
    }))
  }

  function fallbackTargets() {
    const controllerKeys = new Set(Object.entries(state.router.controllers).map(([entryBackend, id]) => sessionRefKey(entryBackend, id)))
    return Object.entries(targetCatalogs()).flatMap(([entryBackend, threads]) => (threads || []).flatMap((thread) => {
      const key = sessionRefKey(entryBackend, thread.id)
      return key && !controllerKeys.has(key) && !thread.archived && !thread.ephemeral
        ? [{ key, backend: entryBackend, thread }]
        : []
    }))
  }

  function renderFallbacks({ capture = false } = {}) {
    if (!runtime.editor) return
    if (capture) captureFallbacks()
    const targets = fallbackTargets()
    const options = (selected) => {
      const known = targets.some((target) => target.key === selected)
      return `<option value="">${t('Select a fallback session')}</option>${!known && selected ? `<option value="${escapeHtml(selected)}" selected>${t('No longer available')} · ${escapeHtml(selected)}</option>` : ''}${targets.map(({ key, backend: targetBackend, thread }) => `<option value="${escapeHtml(key)}"${key === selected ? ' selected' : ''}>[${backendDescriptor(targetBackend).tag}] ${escapeHtml(threadTitle(thread))} — ${escapeHtml(thread.cwd || t('Project directory not recorded'))}</option>`).join('')}`
    }
    const container = element('router-fallbacks')
    container.innerHTML = runtime.editor.fallbacks.length
      ? runtime.editor.fallbacks.map((entry, index) => `<section class="router-fallback-card" data-router-fallback-index="${index}">
        <header><strong>${t('Fallback target {index}', { index: index + 1 })}</strong><button class="icon-button router-remove-fallback" type="button" title="${t('Remove fallback')}" aria-label="${t('Remove fallback')}">×</button></header>
        <label class="field"><span>${t('Target session')}</span><select class="router-fallback-session">${options(entry.sessionKey)}</select></label>
        <label class="field"><span>${t('Fallback condition')}</span><textarea class="router-fallback-condition" rows="2">${escapeHtml(entry.condition || DEFAULT_FALLBACK_CONDITION)}</textarea></label>
      </section>`).join('')
      : `<div class="router-fallback-empty"><strong>${t('No fallback target configured')}</strong><small>${t('When there is no exact match, the Router will choose the closest regular session.')}</small></div>`
    container.querySelectorAll('.router-remove-fallback').forEach((button) => button.addEventListener('click', () => {
      captureFallbacks()
      runtime.editor.fallbacks.splice(Number(button.closest('.router-fallback-card').dataset.routerFallbackIndex), 1)
      renderFallbacks()
    }))
    element('router-add-fallback').disabled = runtime.editor.fallbacks.length >= 3 || !targets.length
  }

  function addFallback() {
    if (!runtime.editor || runtime.editor.fallbacks.length >= 3) return
    captureFallbacks()
    const used = new Set(runtime.editor.fallbacks.map((entry) => entry.sessionKey))
    const target = fallbackTargets().find((entry) => !used.has(entry.key))
    runtime.editor.fallbacks.push({ sessionKey: target?.key || '', condition: DEFAULT_FALLBACK_CONDITION })
    renderFallbacks()
  }

  async function saveSettings(event) {
    event.preventDefault()
    captureFallbacks()
    const fallbacks = (runtime.editor?.fallbacks || []).filter((entry) => entry.sessionKey)
    const errorElement = element('router-error')
    if (new Set(fallbacks.map((entry) => entry.sessionKey)).size !== fallbacks.length) {
      errorElement.textContent = t('The same session cannot be configured as a fallback more than once.')
      errorElement.classList.remove('hidden')
      return
    }
    const button = element('router-form').querySelector('.primary-button')
    button.disabled = true
    errorElement.classList.add('hidden')
    try {
      const controllerBackend = state.router.controllerBackend
      const threadId = await ensureManagedSession(controllerBackend)
      const controllers = { ...state.router.controllers, [controllerBackend]: threadId }
      const previousRouter = state.router
      state.router = normalizeThreadRouter({ controllerBackend, controllers, fallbacks })
      try { await persistPreferences() } catch (error) {
        state.router = previousRouter
        throw error
      }
      closeDialog()
      renderThreadList()
      renderWorkspace()
      notify(t('Router settings saved'))
    } catch (error) {
      errorElement.textContent = error.message
      errorElement.classList.remove('hidden')
    } finally {
      button.disabled = false
    }
  }

  async function ensureManagedSession(controllerBackend = state.router.controllerBackend) {
    if (!dispatch.supports(controllerBackend)) throw new Error(t('The Router backend is currently unavailable.'))
    const cwd = (await loadBackendInfo(controllerBackend))?.routerWorkspace
    if (!cwd) throw new Error(t('Unable to determine the Studio Router working directory.'))
    const routerId = state.router.controllers[controllerBackend]
    const existing = managedRouterThread(state.threadsByBackend[controllerBackend], routerId, cwd)
    if (existing) return existing.id
    if (routerId) {
      try {
        const result = await dispatchRpc(controllerBackend, 'thread/read', { threadId: routerId, includeTurns: false })
        const recoveredCatalog = recoverManagedRouterCatalog(state.threadsByBackend[controllerBackend], routerId, cwd, result?.thread)
        const recovered = managedRouterThread(recoveredCatalog, routerId, cwd)
        if (recovered) {
          state.threadsByBackend[controllerBackend] = recoveredCatalog
          if (controllerBackend === state.backend) state.threads = recoveredCatalog
          return recovered.id
        }
      } catch (error) {
        throw new Error(t('Unable to read the configured Router session. Studio will keep its existing Router ID to avoid creating a duplicate. {message}', { message: error.message }))
      }
      throw new Error(t('The configured Router session does not match the dedicated workspace. Studio will not create a replacement automatically.'))
    }
    if (!shouldCreateManagedRouter(routerId)) throw new Error(t('A Router ID already exists. Studio will not create a replacement automatically.'))
    const result = await dispatchRpc(controllerBackend, 'thread/start', {
      cwd,
      name: 'Thread Router',
      approvalPolicy: 'never',
      sandbox: 'read-only',
    })
    if (!result?.thread?.id) throw new Error(t('Unable to create the system Router session.'))
    if (isCodexBackend(controllerBackend)) await dispatchRpc(controllerBackend, 'thread/name/set', { threadId: result.thread.id, name: 'Thread Router' })
    mergeThread(controllerBackend, { ...result.thread, name: 'Thread Router' })
    state.router = normalizeThreadRouter({
      ...state.router,
      controllers: { ...state.router.controllers, [controllerBackend]: result.thread.id },
    })
    persistPreferences()
    return result.thread.id
  }

  function renderDecisionDebug(turn) {
    const raw = finalAgentText(turn).slice(0, 16 * 1024)
    return raw ? `<details class="router-decision-debug"><summary>${t('View raw decision')}</summary><pre>${escapeHtml(raw)}</pre></details>` : ''
  }

  return {
    bind,
    completeTurn,
    currentCandidates,
    ensureManagedSession,
    isThread,
    openTarget,
    removeSession,
    renderTurn,
    startTurn,
  }
}

function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  })[character])
}
