import type { CodexNotification, CodexParams, CodexViewModel } from './codex-types.mjs'
import type { BackgroundState } from './background-sessions.mjs'
import type { ManagedThread } from './session-operations.mjs'
import { createCodexViewModel, hydrateCodexThread, applyCodexNotification } from './codex-native.mjs'
import { codexLifecycleEvent } from './codex-lifecycle-diagnostics.mjs'
export interface ActiveMessage extends CodexNotification {
  params?: CodexParams & {
    state?: string; generation?: number | string; clientCapabilities?: Record<string, unknown>
    initialization?: unknown; binary?: string; reason?: string; line?: string; skipped?: number
    thread?: ManagedThread & { ephemeral?: boolean }; threadName?: string; tool?: string
    questions?: { question?: string }[]
  }
}
export interface ActiveConnectionState extends BackgroundState {
  socketGeneration: number
  appServerCapabilities: Record<string, unknown>; appServerInitialization: unknown
  threadLoads: Map<string, Promise<unknown>>
  environmentProfile: { configured?: boolean; root: string; revision?: string } | null
  environmentProfileSelectionRoot: string
  hiddenCodexThreads: Set<string>; attentionThreads: Set<string>
  threads: ManagedThread[]
  skillCatalog: { cwd: string | null; skills: unknown[]; request: unknown; loaded: boolean }
}
export interface ActiveConnectionServices {
  $: (selector: string) => { textContent: string | null; value: string; selectionStart: number | null }
  backendDescriptor: (backend: string) => { name: string }
  codexBackendsNeedingRestartRecovery: Set<string>
  sessionDispatch: { clearPrepared: (backend: string) => void }
  clearStartedThreadsForBackend: (backend: string, reason: string) => void
  setBackendState: (kind: string, label: string, caption: string) => void
  setNativeError: (message: string | null) => void
  loadBackendModels: () => Promise<unknown>
  loadThreads: (options: { applyCachedEnvironment: boolean }) => Promise<unknown>
  sessionManagement: { archive: { isOpen: () => boolean; markStale: (options?: { reload: boolean }) => void; remove: (backend: string, id: string | undefined) => void } }
  threadCatalogKey: (backend: string, id: string) => string
  freshThreadModel: (backend: string, id: string, options: { reconnectValidation: boolean }) => unknown
  selectedThread: () => ManagedThread | null
  refreshSelectedThread: (options: { quiet: boolean; environmentRoot?: string; environmentRevision?: string }) => Promise<unknown>
  backendSelectionLoads: Map<string, Promise<unknown>>
  handleThreadCatalogFailure: (backend: string, generation: number, error: unknown) => void
  reportSessionLifecycle: (phase: string, details: Record<string, unknown>) => void
  isArchivedPreview: () => boolean
  toast: (message: string, kind?: string) => void
  t: (text: string, values?: Record<string, string | number>) => string
  reportClientError: (error: unknown) => void
  sessionMap: { captureWorkerResponse: (message: ActiveMessage) => unknown; handleToolCall: (message: ActiveMessage) => unknown }
  pendingRpcRequests: { settle: (message: unknown) => boolean }
  handleCodexLifecycleNotification: (backend: string, message: ActiveMessage) => boolean
  captureStructuredUtilityNotification: (backend: string, message: ActiveMessage) => boolean
  sessionRefKey: (backend: string, id: string | undefined) => string
  mergeThreadMetadata: (thread: ManagedThread) => void
  renderWorkspace: () => void; renderThreadList: () => void; renderTranscript: () => void; renderComposerState: () => void
  hiddenUtilityThread: (backend: string, thread: { id: string | undefined }) => boolean
  forgetStartedThread: (backend: string, id: string | undefined, reason: string) => unknown
  persistSessionPin: (key: string, value: boolean) => Promise<unknown>
  showError: (error: unknown) => void
  deletePersistedSessionState: (key: string) => unknown
  threadRouter: { removeSession: (backend: string, id: string | undefined) => boolean }
  discardComposerSessionState: (backend: string, id: string | undefined) => void
  invalidateThreadModel: (backend: string, id: string | undefined) => void
  persistPreferences: () => unknown
  composerTrigger: (text: string, cursor: number | null) => { type: string } | null
  searchComposerSkills: (trigger: { type: string }) => unknown
  observeCodexTurnLatency: (message: ActiveMessage) => void
  codexNotificationModel: (message: ActiveMessage, backend: string) => CodexViewModel | null
  sendRaw: (message: { id: string | number; error: { code: number; message: string } }) => void
  notifyDesktop: (title: string, body: string) => void
  markCachedModelValidated: (backend: string, model: CodexViewModel) => void
  reportCodexLifecycleNotification: (backend: string, message: ActiveMessage, details?: Record<string, unknown>) => void
  sessionResources: { invalidate: (backend: string, id: string | null) => void }
  transcriptUpdateKind: (method: string | undefined) => string
  queueStreamingItemPatch: (params: CodexParams | undefined) => void
  replaceCompletedItem: (params: CodexParams | undefined) => void
  replaceRenderedTurn: (id: string) => boolean
  rpc: (method: string, params: Record<string, unknown>) => Promise<{ thread: ManagedThread }>
  cacheThreadModel: (backend: string, id: string, model: CodexViewModel) => void
  threadTitle: (thread: ManagedThread) => string
}
export function createActiveCodexConnection(state: ActiveConnectionState, services: ActiveConnectionServices) {
  const { $, backendDescriptor, codexBackendsNeedingRestartRecovery, sessionDispatch, clearStartedThreadsForBackend, setBackendState, setNativeError, loadBackendModels, loadThreads, sessionManagement, threadCatalogKey, freshThreadModel, selectedThread, refreshSelectedThread, backendSelectionLoads, handleThreadCatalogFailure, reportSessionLifecycle, isArchivedPreview, toast, t, reportClientError, sessionMap, pendingRpcRequests, handleCodexLifecycleNotification, captureStructuredUtilityNotification, sessionRefKey, mergeThreadMetadata, renderWorkspace, renderThreadList, renderTranscript, renderComposerState, hiddenUtilityThread, forgetStartedThread, persistSessionPin, showError, deletePersistedSessionState, threadRouter, discardComposerSessionState, invalidateThreadModel, persistPreferences, composerTrigger, searchComposerSkills, observeCodexTurnLatency, codexNotificationModel, sendRaw, notifyDesktop, markCachedModelValidated, reportCodexLifecycleNotification, sessionResources, transcriptUpdateKind, queueStreamingItemPatch, replaceCompletedItem, replaceRenderedTurn, rpc, cacheThreadModel, threadTitle } = services
function handleAppServerMessage(message: ActiveMessage) {
  const backend = state.backend
  const descriptor = backendDescriptor(backend)
  if (message.method === 'studio/appServer/status') {
    const status = message.params?.state
    if (status === 'ready') {
      const firstReady = !state.ready
      const previousGeneration = state.appServerGenerations[backend]
      const nextGeneration = message.params?.generation ?? previousGeneration
      const appServerRestarted = codexBackendsNeedingRestartRecovery.delete(backend)
        || (previousGeneration != null && nextGeneration !== previousGeneration)
      if (appServerRestarted) {
        sessionDispatch.clearPrepared(backend)
        clearStartedThreadsForBackend(backend, 'app-server-restarted')
      }
      state.appServerGenerations[backend] = nextGeneration
      state.appServerCapabilities = { ...(message.params?.clientCapabilities || {}) }
      state.appServerInitialization = message.params?.initialization || null
      state.ready = true
      setBackendState('online', `${descriptor.name} App Server`, 'Native structured connection')
      $('#native-connection').textContent = 'Connected'
      setNativeError(null)
      loadBackendModels().catch((error) => console.debug('Unable to load Codex models', error))
      if (firstReady) {
        const socketGeneration = state.socketGeneration
        const cachedModelsBeforeCatalog = new Map(state.threadModels)
        const selectionLoad = loadThreads({ applyCachedEnvironment: !appServerRestarted }).then(async () => {
          if (state.backend !== backend
            || state.socketGeneration !== socketGeneration
            || sessionManagement.archive.isOpen()
            || !state.selectedId) return
          const selectedId = state.selectedId
          const selectedKey = threadCatalogKey(backend, selectedId)
          // The operator may have selected another session while the catalog
          // was loading. Let that session's resume finish before deciding
          // whether reconnect recovery still needs an authoritative read.
          const activeHistory = state.threadLoads.get(selectedKey)
          if (activeHistory) {
            try { await activeHistory }
            catch (error) { console.warn('Selected history load failed before reconnect recovery', error) }
          }
          if (state.backend !== backend
            || state.socketGeneration !== socketGeneration
            || sessionManagement.archive.isOpen()
            || state.selectedId !== selectedId) return
          // loadThreads() may already have resumed a stale or uncached selection.
          // Its cache replacement is proof that this generation has supplied a
          // complete history, so do not immediately read the same history again.
          if (state.threadModels.get(selectedKey) !== cachedModelsBeforeCatalog.get(selectedKey)) return
          // Reopening only the browser-side WebSocket does not invalidate a
          // history that is still fresh for the same App Server process.
          if (!appServerRestarted && freshThreadModel(backend, selectedId, { reconnectValidation: true })) return
          const root = selectedThread()?.cwd || ''
          const profile = state.environmentProfile
          const environmentRoot = appServerRestarted
            && profile?.configured
            && state.environmentProfileSelectionRoot === root
            ? profile.root
            : ''
          await refreshSelectedThread({
            quiet: true,
            environmentRoot,
            environmentRevision: profile?.revision || '',
          })
        })
        backendSelectionLoads.set(backend, selectionLoad)
        selectionLoad
          .catch((error) => handleThreadCatalogFailure(backend, socketGeneration, error))
          .finally(() => {
            if (backendSelectionLoads.get(backend) === selectionLoad) backendSelectionLoads.delete(backend)
          })
      }
    } else if (status === 'starting') {
      setBackendState('checking', `Starting ${descriptor.name}`, message.params?.binary || 'App Server')
    } else if (status === 'error' || status === 'stopped') {
      sessionDispatch.clearPrepared(backend)
      const reason = message.params?.message || message.params?.reason || 'App Server stopped'
      setBackendState('error', `${descriptor.name} Unavailable`, reason)
      setNativeError(reason)
    }
    return
  }
  if (message.method === 'studio/appServer/log') {
    console.debug(`${backend} app-server`, message.params?.line)
    return
  }
  if (message.method === 'studio/appServer/lagged') {
    const skipped = Number(message.params?.skipped || 0)
    const threadId = state.selectedId
    const socketGeneration = state.socketGeneration
    const cachedRunningThreadIds = [...state.threadModels.entries()]
      .filter(([key, cached]) => key.startsWith(`${backend}:`)
        && (cached.model?.activeTurnId || cached.model?.status === 'running'))
      .map(([key]) => key.slice(backend.length + 1))
      .join(',')
    reportSessionLifecycle('codex-event-lag', {
      backend,
      selectedId: threadId || '',
      skipped,
      cachedRunningThreadIds,
      socketGeneration,
    })
    if (isArchivedPreview()) return
    if (!threadId) {
      toast(t('The UI missed {count} App Server events', { count: skipped }), 'error')
      return
    }
    setNativeError(`The interface missed ${skipped} App Server events and is resynchronizing the current session…`)
    resynchronizeSelectedThreadAfterLag({ backend, threadId, socketGeneration, skipped }).catch((error) => {
      reportClientError(error)
      if (state.backend === backend && state.selectedId === threadId && state.socketGeneration === socketGeneration) {
        setNativeError(t('Unable to resynchronize the current {backend} session: {message}', {
          backend: backendDescriptor(backend).name,
          message: error instanceof Error ? error.message : String(error),
        }))
      }
    })
    return
  }
  if (message.method?.startsWith('studio/appServer/')) {
    const error = message.params?.message || message.method
    setNativeError(error)
    return
  }

  if (message.id != null && !message.method) {
    sessionMap.captureWorkerResponse(message)
    pendingRpcRequests.settle(message)
    return
  }

  if (codexLifecycleEvent(message)) {
    handleCodexLifecycleNotification(backend, message)
    return
  }

  if (captureStructuredUtilityNotification(backend, message)) return

  if (message.method === 'thread/started' && message.params?.thread) {
    if (message.params.thread.ephemeral || state.hiddenCodexThreads.has(sessionRefKey(state.backend, message.params.thread.id))) return
    mergeThreadMetadata(message.params.thread)
    renderWorkspace()
    return
  }
  if (message.method === 'thread/name/updated') {
    const thread = state.threads.find((candidate) => candidate.id === message.params?.threadId)
    if (thread) thread.name = message.params!.threadName!
    renderThreadList()
    renderWorkspace()
    return
  }
  if ((message.method === 'thread/archived' || message.method === 'thread/deleted')
    && hiddenUtilityThread(state.backend, { id: message.params?.threadId })) return
  if (message.method === 'thread/archived' || message.method === 'thread/deleted') {
    const backend = state.backend
    const threadId = message.params?.threadId
    const sessionKey = `${backend}:${threadId}`
    forgetStartedThread(backend, threadId, message.method === 'thread/deleted' ? 'deleted' : 'archived')
    sessionManagement.archive.markStale()
    if (message.method === 'thread/deleted') {
      sessionManagement.archive.remove(backend, threadId)
    }
    state.threads = state.threads.filter((thread) => thread.id !== threadId)
    state.threadsByBackend[backend] = state.threads
    if (state.pinnedSessions.delete(sessionKey) && message.method === 'thread/archived') {
      persistSessionPin(sessionKey, false).catch(showError)
    }
    if (message.method === 'thread/deleted') {
      const deletedKey = sessionKey
      delete state.annotationDrafts[deletedKey]
      delete state.annotationAdditional[deletedKey]
      delete state.openingMessages[deletedKey]
      delete state.turnOptions[deletedKey]
      delete state.messageQueues[deletedKey]
      state.pausedMessageQueues.delete(deletedKey)
      state.messageQueueErrors.delete(deletedKey)
      deletePersistedSessionState(deletedKey)
    }
    threadRouter.removeSession(backend, threadId)
    discardComposerSessionState(backend, threadId)
    invalidateThreadModel(backend, threadId)
    persistPreferences()
    if (state.selectedId === threadId) {
      state.selectedId = null
      state.selectedByBackend[backend] = null
      state.model = createCodexViewModel()
      persistPreferences()
    }
    renderThreadList()
    renderWorkspace()
    return
  }

  if (message.method === 'thread/unarchived') {
    sessionManagement.archive.markStale({ reload: true })
    return
  }

  if (message.method === 'skills/changed') {
    state.skillCatalog = { cwd: null, skills: [], request: null, loaded: false }
    const input = $('#composer-input')
    const trigger = composerTrigger(input.value, input.selectionStart)
    if (trigger?.type === 'skill') searchComposerSkills(trigger)
    return
  }

  observeCodexTurnLatency(message)

  if (message.id != null && message.method) {
    if (message.method === 'item/tool/call' && message.params?.tool === 'update_session_map') {
      sessionMap.handleToolCall(message)
      return
    }
    const targetModel = codexNotificationModel(message, backend)
    if (!targetModel) {
      if (message.method === 'item/tool/requestUserInput' || message.method === 'mcpServer/elicitation/request') {
        captureOffscreenInteraction(message).catch((error) => reportClientError(error))
      }
      return
    }
    if (!applyCodexNotification(targetModel, message)) {
      sendRaw({ id: message.id, error: { code: -32601, message: `Studio does not support ${message.method}` } })
      toast(t('Codex requested an unsupported interaction: {method}', { method: message.method }), 'error')
      return
    }
    if (message.method === 'item/tool/requestUserInput' || message.method === 'mcpServer/elicitation/request') {
      notifyDesktop(t('Codex is waiting for your input'), message.params?.questions?.[0]?.question || message.params?.message || selectedThread()?.name || '')
    }
    markCachedModelValidated(backend, targetModel)
    if (targetModel !== state.model) return
    renderTranscript()
    return
  }

  const targetModel = codexNotificationModel(message, backend)
  if (!targetModel) {
    reportCodexLifecycleNotification(backend, message)
    return
  }
  const beforeModelStatus = targetModel.status || ''
  const beforeActiveTurnId = targetModel.activeTurnId || ''
  const applied = applyCodexNotification(targetModel, message)
  if (!applied) {
    reportCodexLifecycleNotification(backend, message, {
      targetModel,
      beforeModelStatus,
      beforeActiveTurnId,
    })
    return
  }
  markCachedModelValidated(backend, targetModel)
  if (message.method === 'item/completed') {
    sessionResources.invalidate(
      backend,
      message.params?.threadId || targetModel.threadId || state.selectedId,
    )
  }
  if (targetModel !== state.model) {
    reportCodexLifecycleNotification(backend, message, {
      targetModel,
      applied,
      beforeModelStatus,
      beforeActiveTurnId,
    })
    return
  }
  const updateKind = transcriptUpdateKind(message.method)
  if (updateKind === 'stream') queueStreamingItemPatch(message.params)
  else if (updateKind === 'item') replaceCompletedItem(message.params)
  else if (updateKind === 'full') {
    const turnId = message.params?.turnId || message.params?.turn?.id
    if (!turnId || !replaceRenderedTurn(turnId)) renderTranscript()
  }
  renderComposerState()
  reportCodexLifecycleNotification(backend, message, {
    targetModel,
    applied,
    beforeModelStatus,
    beforeActiveTurnId,
  })
}

async function resynchronizeSelectedThreadAfterLag({ backend, threadId, socketGeneration, skipped }: { backend: string; threadId: string; socketGeneration: number; skipped: number }) {
  const activeHistory = state.threadLoads.get(threadCatalogKey(backend, threadId))
  if (activeHistory) {
    try { await activeHistory }
    catch (error) { console.warn('Selected history load failed before lag recovery', error) }
  }
  if (state.backend !== backend
    || state.selectedId !== threadId
    || state.socketGeneration !== socketGeneration
    || !state.ready) return false
  const refreshed = await refreshSelectedThread({ quiet: true })
  if (!refreshed
    || state.backend !== backend
    || state.selectedId !== threadId
    || state.socketGeneration !== socketGeneration) return false
  setNativeError(null)
  toast(t('Resynchronized the session from {backend}', { backend: backendDescriptor(backend).name }))
  console.debug('Recovered missed App Server events', { backend, threadId, skipped })
  return true
}

async function captureOffscreenInteraction(message: ActiveMessage) {
  const backend = state.backend
  const threadId = String(message.params?.threadId || '')
  if (!threadId) return
  const result = await rpc('thread/read', { threadId, includeTurns: true })
  const model = createCodexViewModel()
  hydrateCodexThread(model, result.thread)
  applyCodexNotification(model, message)
  cacheThreadModel(backend, threadId, model)
  state.attentionThreads.add(threadCatalogKey(backend, threadId))
  persistPreferences()
  renderThreadList()
  notifyDesktop(t('Codex is waiting for your input'), message.params?.questions?.[0]?.question || message.params?.message || threadTitle(result.thread))
}
return { handleAppServerMessage, resynchronizeSelectedThreadAfterLag, captureOffscreenInteraction }
}
