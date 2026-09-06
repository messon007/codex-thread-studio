import type { CodexViewModel, CodexNotification, CodexParams } from './codex-types.mjs'
import type { OpenCodeEvent } from './opencode-event-types.mjs'
import type { BufferedOpenCodeEvent, HistoryReplayOptions } from './opencode-history-types.mjs'
import type { LifecycleEvent } from './notification-types.mjs'
import type { ManagedThread } from './session-operations.mjs'
import { createCodexViewModel, hydrateCodexThread, applyCodexNotification } from './codex-native.mjs'
import { applyOpenCodeEvent, replayOpenCodeEventsAfterHistory, createOpenCodeLoopGuard } from './opencode-native.mjs'
import { transcriptModelRevision } from './model-revision.mjs'

export interface BackgroundModel extends CodexViewModel { historyComplete?: boolean }
export interface BackgroundThread extends ManagedThread {
  status?: string
  messageTurns?: Record<string, string>
  messageRoles?: Record<string, string | undefined>
  messageItems?: Record<string, string[]>
  messageErrors?: Record<string, { turnId: string; message: string }>
}
export interface BackgroundHistory {
  thread: BackgroundThread
  historyComplete?: boolean
  historyMessageSnapshots?: HistoryReplayOptions['messageSnapshots']
  historyStatusSequence: number
}
export interface BackgroundNotification extends CodexNotification {
  params?: CodexParams & { state?: string; generation?: string | number; binary?: string; reason?: string; skipped?: number }
}
export interface BackgroundState {
  backend: string; selectedId: string | null; ready: boolean
  model: BackgroundModel
  threadModels: Map<string, { model: BackgroundModel; validatedAt: number }>
  threadsByBackend: Record<string, BackgroundThread[]>
  appServerGenerations: Record<string, string | number | null | undefined>
  backendStates: Record<string, { kind: string; label: string; caption: string }>
  selectedByBackend: Record<string, string | null>
  pinnedSessions: Set<string>
  annotationDrafts: Record<string, unknown>; annotationAdditional: Record<string, unknown>
  openingMessages: Record<string, unknown>; turnOptions: Record<string, unknown>; messageQueues: Record<string, unknown>
  pausedMessageQueues: Set<string>; messageQueueErrors: Map<string, string>
}
export interface BackgroundServices {
  $: (selector: string) => { open: boolean }
  getBackendIds: () => string[]
  codexBackendsNeedingRestartRecovery: Set<string>
  scheduleStudioIdleWork: (callback: () => unknown, options: { delay: number; timeout: number }) => unknown
  refreshBackendCatalog: (backend: string) => Promise<unknown>
  refreshSelectedThread: (options: { quiet: boolean }) => Promise<unknown>
  threadStatus: (thread: BackgroundThread | undefined) => string
  handleQueuedTurnCompletion: (ref: { backend: string; id: string }, status: string) => void
  backendDescriptor: (backend: string) => { name: string }
  sessionDispatch: { clearPrepared: (backend: string) => void; read: (ref: { backend: string; id: string }) => Promise<BackgroundHistory> }
  clearStartedThreadsForBackend: (backend: string, reason: string) => void
  isCodexBackend: (backend: string) => boolean
  renderConnectionsDialog: () => void; renderBackendDialog: () => void
  reportSessionLifecycle: (phase: string, details: Record<string, unknown>) => void
  markCachedModelUnvalidated: (backend: string, model: BackgroundModel) => void
  markCachedModelValidated: (backend: string, model: BackgroundModel) => void
  threadCatalogKey: (backend: string, id: string) => string
  loadCodexHistoryForBackground: (backend: string, id: string, cache: { model: BackgroundModel }) => Promise<{ result: BackgroundHistory; historyMode: string; tail?: { pageCount?: number; appendedTurnCount?: number } | null }>
  cachedThreadModel: (backend: string, id: string) => { model: BackgroundModel } | null
  mergeThreadIntoCatalog: (backend: string, thread: BackgroundThread) => void
  cacheThreadModel: (backend: string, id: string, model: BackgroundModel, options?: { historyEpoch: number | null }) => void
  claimCodexLifecycleNotification: (backend: string, message: BackgroundNotification) => LifecycleEvent | null
  captureStructuredUtilityNotification: (backend: string, message: BackgroundNotification) => boolean
  codexLifecycleThreadId: (backend: string, event: LifecycleEvent) => string
  scheduleStartedThreadCatalogConfirmation: (backend: string, id: string) => void
  updateCodexCatalogActivity: (backend: string, message: BackgroundNotification, event: LifecycleEvent, id: string) => unknown
  observeCodexTurnLatency: (message: BackgroundNotification, backend: string) => void
  codexNotificationModel: (message: BackgroundNotification, backend: string) => BackgroundModel | null
  reportCodexLifecycleNotification: (backend: string, message: BackgroundNotification, details?: Record<string, unknown>) => void
  sessionResources: { invalidate: (backend: string, id: string | null) => void }
  notifyDesktop: (title: string, body: string, ref: { backend: string; id: string | null }) => void
  t: (text: string) => string
  threadTitle: (thread: BackgroundThread) => string
  threadRouter: { completeTurn: (params: { backend: string; turnId: string; model: BackgroundModel; turn: CodexParams['turn'] }) => Promise<unknown>; removeSession: (backend: string, id: string | undefined) => boolean }
  transcriptUpdateKind: (method: string | undefined) => string
  replaceRenderedTurn: (id: string) => boolean
  renderTranscript: () => void; renderComposerState: () => void; renderWorkspace: () => void
  sessionMap: { processInlineUpdate: (backend: string, id: string | null, model: BackgroundModel, turnId: string) => Promise<unknown> }
  openCodeEventThreadId: (payload: OpenCodeEvent) => string
  bufferOpenCodeHistoryEvent: (id: string, event: OpenCodeEvent) => void
  openCodeLoopGuard: ReturnType<typeof createOpenCodeLoopGuard>
  abortRepeatedOpenCodeTerminalLoop: (loop: NonNullable<ReturnType<ReturnType<typeof createOpenCodeLoopGuard>['observe']>>) => unknown
  hiddenUtilityThread: (backend: string, thread: { id?: string }) => boolean
  markUtilityThreadHidden: (backend: string, id: string) => void
  scheduleOpenCodeListRefresh: () => void
  sessionRefKey: (backend: string, id: string | undefined) => string
  forgetStartedThread: (backend: string, id: string | undefined, reason: string) => unknown
  deletePersistedSessionState: (key: string) => unknown
  discardComposerSessionState: (backend: string, id: string | undefined) => void
  invalidateThreadModel: (backend: string, id: string | undefined) => void
  persistPreferences: () => unknown
  openCodeCompletionSignal: (event: OpenCodeEvent) => boolean
  scheduleOpenCodeStatusReconciliation: (id: string) => void
  updateOpenCodeCatalogActivity: (payload: OpenCodeEvent, id: string) => unknown
  queueStreamingItemPatch: (params: { turnId: string | null | undefined; itemId: string | undefined }) => void
  getOpenCodeHistoryEpoch: () => number
  beginOpenCodeHistoryEventBuffer: (id: string) => BufferedOpenCodeEvent[]
  openCodeHistoryIncludesDeletion: (events: BufferedOpenCodeEvent[], id: string) => boolean
  hydrateOpenCodeModelMetadata: (thread: BackgroundThread, model: BackgroundModel, complete?: boolean) => void
  markTranscriptHistoryReady: (options: { complete: boolean }) => void
  transcriptCaptureSuppressedKeys: Set<string>
  endOpenCodeHistoryEventBuffer: (id: string, events: BufferedOpenCodeEvent[]) => void
}
export function createBackgroundSessions(state: BackgroundState, services: BackgroundServices) {
  const { $, getBackendIds, codexBackendsNeedingRestartRecovery, scheduleStudioIdleWork, refreshBackendCatalog, refreshSelectedThread, threadStatus, handleQueuedTurnCompletion, backendDescriptor, sessionDispatch, clearStartedThreadsForBackend, isCodexBackend, renderConnectionsDialog, renderBackendDialog, reportSessionLifecycle, markCachedModelUnvalidated, markCachedModelValidated, threadCatalogKey, loadCodexHistoryForBackground, cachedThreadModel, mergeThreadIntoCatalog, cacheThreadModel, claimCodexLifecycleNotification, captureStructuredUtilityNotification, codexLifecycleThreadId, scheduleStartedThreadCatalogConfirmation, updateCodexCatalogActivity, observeCodexTurnLatency, codexNotificationModel, reportCodexLifecycleNotification, sessionResources, notifyDesktop, t, threadTitle, threadRouter, transcriptUpdateKind, replaceRenderedTurn, renderTranscript, renderComposerState, renderWorkspace, sessionMap, openCodeEventThreadId, bufferOpenCodeHistoryEvent, openCodeLoopGuard, abortRepeatedOpenCodeTerminalLoop, hiddenUtilityThread, markUtilityThreadHidden, scheduleOpenCodeListRefresh, sessionRefKey, forgetStartedThread, deletePersistedSessionState, discardComposerSessionState, invalidateThreadModel, persistPreferences, openCodeCompletionSignal, scheduleOpenCodeStatusReconciliation, updateOpenCodeCatalogActivity, queueStreamingItemPatch, getOpenCodeHistoryEpoch, beginOpenCodeHistoryEventBuffer, openCodeHistoryIncludesDeletion, hydrateOpenCodeModelMetadata, markTranscriptHistoryReady, transcriptCaptureSuppressedKeys, endOpenCodeHistoryEventBuffer } = services
  let openCodeLifecycleReconciliation: Promise<void> | null = null
  const codexOffscreenHistoryRefreshes = new Map<string, Promise<void>>()
  const codexOffscreenHistoryRefreshRetries = new Map<string, boolean>()
function scheduleOpenCodeLifecycleReconciliation(): Promise<void> {
  if (openCodeLifecycleReconciliation) return openCodeLifecycleReconciliation
  openCodeLifecycleReconciliation = new Promise<void>((resolve) => {
    scheduleStudioIdleWork(resolve, { delay: 300, timeout: 1_500 })
  }).then(async () => {
    await refreshBackendCatalog('opencode')
    const catalog = state.threadsByBackend.opencode || []
    const staleRunningIds = [...state.threadModels.entries()]
      .filter(([key, cached]) => key.startsWith('opencode:')
        && (cached.model?.activeTurnId || cached.model?.status === 'running'))
      .map(([key]) => key.slice('opencode:'.length))
      .filter((id) => {
        const status = threadStatus(catalog.find((thread) => thread.id === id))
        return status !== 'active' && status !== 'running' && status !== 'inProgress'
      })
    for (const id of staleRunningIds) {
      let model
      if (state.backend === 'opencode' && state.selectedId === id && state.ready) {
        await refreshSelectedThread({ quiet: true })
        model = state.model
      } else {
        model = await ensureSessionModel({ backend: 'opencode', id })
      }
      const lastTurn = model?.turns?.at(-1)
      if (!model?.activeTurnId && lastTurn) {
        handleQueuedTurnCompletion(
          { backend: 'opencode', id },
          lastTurn.status || model.status || 'completed',
        )
      }
    }
  }).catch((error) => {
    console.debug('Unable to reconcile OpenCode after an event-stream gap', error)
  }).finally(() => {
    openCodeLifecycleReconciliation = null
  })
  return openCodeLifecycleReconciliation
}

function invalidateCodexBackendModelValidation(backend: string) {
  for (const [key, cached] of state.threadModels) {
    if (key.startsWith(`${backend}:`)) cached.validatedAt = 0
  }
}

function handleInactiveCodexAppServerStatus(backend: string, message: BackgroundNotification) {
  if (backend === state.backend) return
  const descriptor = backendDescriptor(backend)
  const status = message.params?.state
  const previousGeneration = state.appServerGenerations[backend]
  const nextGeneration = message.params?.generation ?? previousGeneration
  if (status === 'ready') {
    if (previousGeneration != null && nextGeneration !== previousGeneration) {
      sessionDispatch.clearPrepared(backend)
      clearStartedThreadsForBackend(backend, 'app-server-restarted')
      invalidateCodexBackendModelValidation(backend)
      codexBackendsNeedingRestartRecovery.add(backend)
    }
    state.appServerGenerations[backend] = nextGeneration
    state.backendStates[backend] = {
      kind: 'online',
      label: `${descriptor.name} App Server`,
      caption: 'Background lifecycle connection',
    }
  } else if (status === 'starting') {
    state.backendStates[backend] = {
      kind: 'checking',
      label: `Starting ${descriptor.name}`,
      caption: message.params?.binary || 'App Server',
    }
  } else if (status === 'error' || status === 'stopped') {
    sessionDispatch.clearPrepared(backend)
    state.backendStates[backend] = {
      kind: 'error',
      label: `${descriptor.name} Unavailable`,
      caption: message.params?.message || message.params?.reason || 'App Server stopped',
    }
  }
  if ($('#connections-dialog').open) renderConnectionsDialog()
  if ($('#backend-dialog').open) renderBackendDialog()
}

function handleCodexLifecycleLag(backend: string, message: BackgroundNotification) {
  const skipped = Number(message.params?.skipped || 0)
  reportSessionLifecycle('codex-lifecycle-lag', { backend, skipped })
  // The selected backend still has its full event connection and performs its
  // own lag recovery. This stream only needs to reconcile offscreen backends.
  if (backend === state.backend) return
  reconcileCodexLifecycleBackend(backend).catch((error) => {
    console.warn(`Unable to reconcile ${backend} after lifecycle event lag`, error)
  })
}

function scheduleCodexLifecycleReconciliation() {
  scheduleStudioIdleWork(() => Promise.all(
    getBackendIds()
      .filter((backend) => backend !== state.backend && isCodexBackend(backend))
      .map((backend) => reconcileCodexLifecycleBackend(backend).catch((error) => {
        console.debug(`Unable to reconcile reconnected ${backend} lifecycle state`, error)
      })),
  ), { delay: 250, timeout: 1_500 })
}

async function reconcileCodexLifecycleBackend(backend: string) {
  if (!isCodexBackend(backend) || backend === state.backend) return
  await refreshBackendCatalog(backend)
  if (backend === state.backend) return
  const catalog = state.threadsByBackend[backend] || []
  const staleRunning = [...state.threadModels.entries()]
    .filter(([key, cached]) => key.startsWith(`${backend}:`)
      && (cached.model?.activeTurnId || cached.model?.status === 'running'))
    .map(([key, cached]) => ({
      id: key.slice(backend.length + 1),
      model: cached.model,
    }))
    .filter(({ id }) => {
      const status = threadStatus(catalog.find((thread) => thread.id === id))
      return status !== 'active' && status !== 'running' && status !== 'inProgress'
    })
  for (const entry of staleRunning) {
    if (backend === state.backend) return
    markCachedModelUnvalidated(backend, entry.model)
    await refreshOffscreenCodexHistoryAfterCompletion(
      backend,
      entry.id,
      entry.model,
      { advanceQueue: true },
    )
  }
}

function refreshOffscreenCodexHistoryAfterCompletion(
  backend: string,
  threadId: string | null,
  sourceModel: BackgroundModel,
  { advanceQueue = false } = {},
): Promise<void> | undefined {
  if (!threadId || backend === state.backend) return
  const key = threadCatalogKey(backend, threadId)
  if (codexOffscreenHistoryRefreshes.has(key)) {
    codexOffscreenHistoryRefreshRetries.set(
      key,
      Boolean(codexOffscreenHistoryRefreshRetries.get(key) || advanceQueue),
    )
    return codexOffscreenHistoryRefreshes.get(key)
  }
  const sourceRevision = transcriptModelRevision(sourceModel)
  let refresh: Promise<void>
  refresh = (async () => {
    const loaded = await loadCodexHistoryForBackground(backend, threadId, { model: sourceModel })
    if (backend === state.backend) return
    const current = cachedThreadModel(backend, threadId)
    if (current?.model !== sourceModel) return
    if (transcriptModelRevision(sourceModel) !== sourceRevision) {
      if (!sourceModel.activeTurnId && sourceModel.status !== 'running') {
        codexOffscreenHistoryRefreshRetries.set(
          key,
          Boolean(codexOffscreenHistoryRefreshRetries.get(key) || advanceQueue),
        )
      }
      return
    }
    const model: BackgroundModel = createCodexViewModel()
    hydrateCodexThread(model, loaded.result.thread)
    model.historyComplete = loaded.result.historyComplete !== false
    mergeThreadIntoCatalog(backend, loaded.result.thread)
    cacheThreadModel(backend, threadId, model)
    const lastTurn = model.turns.at(-1)
    if (advanceQueue && !model.activeTurnId && lastTurn) {
      handleQueuedTurnCompletion(
        { backend, id: threadId },
        lastTurn.status || model.status || 'completed',
      )
    }
    reportSessionLifecycle('codex-background-history', {
      backend,
      threadId,
      historyMode: loaded.historyMode,
      pageCount: loaded.tail?.pageCount || 0,
      appendedTurnCount: loaded.tail?.appendedTurnCount || 0,
    })
  })().catch((error) => {
    console.debug(`Unable to refresh completed offscreen ${backend} session`, error)
  }).finally(() => {
    if (codexOffscreenHistoryRefreshes.get(key) === refresh) codexOffscreenHistoryRefreshes.delete(key)
    const retryAdvanceQueue = codexOffscreenHistoryRefreshRetries.get(key)
    codexOffscreenHistoryRefreshRetries.delete(key)
    if (retryAdvanceQueue != null && backend !== state.backend) {
      queueMicrotask(() => {
        const current = cachedThreadModel(backend, threadId)?.model
        if (current) refreshOffscreenCodexHistoryAfterCompletion(
          backend,
          threadId,
          current,
          { advanceQueue: retryAdvanceQueue },
        )
      })
    }
  })
  codexOffscreenHistoryRefreshes.set(key, refresh)
  return refresh
}

function handleCodexLifecycleNotification(backend: string, message: BackgroundNotification) {
  const event = claimCodexLifecycleNotification(backend, message)
  if (!event) return false
  if (captureStructuredUtilityNotification(backend, message)) return true

  const threadId = codexLifecycleThreadId(backend, event)
  if ((event.method === 'turn/started' || event.method === 'turn/completed') && threadId) {
    scheduleStartedThreadCatalogConfirmation(backend, threadId)
  }
  updateCodexCatalogActivity(backend, message, event, threadId)
  observeCodexTurnLatency(message, backend)
  if (event.method === 'turn/completed' && threadId) {
    handleQueuedTurnCompletion(
      { backend, id: threadId },
      String(message.params?.turn?.status || message.params?.status || 'completed'),
    )
  }

  const targetModel = codexNotificationModel(message, backend)
  if (!targetModel) {
    reportCodexLifecycleNotification(backend, message)
    return true
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
    return true
  }
  const fullEventCoverage = state.backend === backend
  if (fullEventCoverage) markCachedModelValidated(backend, targetModel)
  else markCachedModelUnvalidated(backend, targetModel)
  if (event.method === 'turn/completed') {
    sessionResources.invalidate(backend, threadId || targetModel.threadId)
    const completedThread = (state.threadsByBackend[backend] || [])
      .find((thread) => thread.id === (threadId || targetModel.threadId))
    notifyDesktop(
      t('Work completed'),
      threadTitle(completedThread || { name: t('Untitled session') }),
      { backend, id: threadId || targetModel.threadId },
    )
    threadRouter.completeTurn({
      backend,
      turnId: event.turnId,
      model: targetModel,
      turn: message.params?.turn,
    }).catch((error) => console.error('Thread Router dispatch failed', error))
    if (!fullEventCoverage) {
      refreshOffscreenCodexHistoryAfterCompletion(backend, threadId || targetModel.threadId, targetModel)
    }
  }

  const selected = state.backend === backend && targetModel === state.model
  if (!selected) {
    reportCodexLifecycleNotification(backend, message, {
      targetModel,
      applied,
      beforeModelStatus,
      beforeActiveTurnId,
    })
    return true
  }
  const updateKind = transcriptUpdateKind(message.method)
  if (updateKind === 'full') {
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
  if (event.method === 'turn/completed') {
    sessionMap.processInlineUpdate(backend, threadId || state.selectedId, targetModel, event.turnId).catch((error) => {
      console.warn('Session Map inline update failed', error)
    })
  }
  return true
}

function handleOpenCodeServerEvent(event: OpenCodeEvent) {
  const payload = event?.payload || event
  if (!payload?.type || payload.type === 'sync' || payload.type === 'server.heartbeat') return
  const eventThreadId = openCodeEventThreadId(payload)
  if (eventThreadId && payload.type === 'session.idle') {
    handleQueuedTurnCompletion({ backend: 'opencode', id: eventThreadId }, 'completed')
  } else if (eventThreadId && (payload.type === 'session.error' || payload.type === 'session.abort')) {
    handleQueuedTurnCompletion({ backend: 'opencode', id: eventThreadId }, 'failed')
  }
  bufferOpenCodeHistoryEvent(eventThreadId, event)
  const repeatedTerminalLoop = openCodeLoopGuard.observe(payload)
  if (repeatedTerminalLoop) abortRepeatedOpenCodeTerminalLoop(repeatedTerminalLoop)
  const eventThread = payload.properties?.info || { id: eventThreadId }
  if (hiddenUtilityThread('opencode', eventThread)) {
    if (eventThreadId) markUtilityThreadHidden('opencode', eventThreadId)
    return
  }
  if (payload.type === 'server.connected') {
    // EventSource.onopen owns the continuity epoch and reconnect refresh.
    return
  }
  if (payload.type.startsWith('session.')) scheduleOpenCodeListRefresh()
  if (payload.type === 'session.deleted') {
    const deletedId = payload.properties?.info?.id || payload.properties?.sessionID
    const deletedKey = sessionRefKey('opencode', deletedId)
    forgetStartedThread('opencode', deletedId, 'deleted')
    state.pinnedSessions.delete(deletedKey)
    delete state.annotationDrafts[deletedKey]
    delete state.annotationAdditional[deletedKey]
    delete state.openingMessages[deletedKey]
    delete state.turnOptions[deletedKey]
    delete state.messageQueues[deletedKey]
    state.pausedMessageQueues.delete(deletedKey)
    state.messageQueueErrors.delete(deletedKey)
    deletePersistedSessionState(deletedKey)
    discardComposerSessionState('opencode', deletedId)
    if (threadRouter.removeSession('opencode', deletedId)) persistPreferences()
    if (deletedId && state.backend === 'opencode' && state.selectedId === deletedId) {
      state.selectedId = null
      state.selectedByBackend.opencode = null
      state.model = createCodexViewModel()
      persistPreferences()
      renderWorkspace()
    }
    invalidateThreadModel('opencode', deletedId)
    return
  }
  const completionSignal = Boolean(eventThreadId && openCodeCompletionSignal(payload))
  if (completionSignal) scheduleOpenCodeStatusReconciliation(eventThreadId)
  updateOpenCodeCatalogActivity(payload, eventThreadId)
  const cached = eventThreadId ? state.threadModels.get(threadCatalogKey('opencode', eventThreadId)) : undefined
  const selectedTarget = state.backend === 'opencode' && eventThreadId === state.selectedId
  const targetModel = selectedTarget
    ? state.model
    : cached?.model
  if (!targetModel) return
  const update = applyOpenCodeEvent(targetModel, event, eventThreadId)
  if (!update.handled) return
  markCachedModelValidated('opencode', targetModel)
  if (completionSignal) sessionResources.invalidate('opencode', eventThreadId)
  if (!selectedTarget || targetModel !== state.model) return
  if (update.kind === 'stream') queueStreamingItemPatch({ turnId: update.turnId, itemId: update.itemId })
  else if (update.kind === 'metadata') {
    renderComposerState()
  } else if (!update.turnId || !replaceRenderedTurn(update.turnId)) renderTranscript()
  renderComposerState()
}

async function ensureSessionModel(ref: { backend: string; id: string }) {
  const key = threadCatalogKey(ref.backend, ref.id)
  const historyEpoch = ref.backend === 'opencode' ? getOpenCodeHistoryEpoch() : null
  const historyEvents = ref.backend === 'opencode' ? beginOpenCodeHistoryEventBuffer(ref.id) : null
  try {
    const result = await sessionDispatch.read(ref)
    if (historyEvents && openCodeHistoryIncludesDeletion(historyEvents, ref.id)) {
      throw new Error(t('This OpenCode session was deleted while its history was loading.'))
    }
    const model = state.backend === ref.backend && state.selectedId === ref.id
      ? state.model
      : state.threadModels.get(key)?.model || createCodexViewModel()
    hydrateCodexThread(model, result.thread)
    if (ref.backend === 'opencode') {
      hydrateOpenCodeModelMetadata(result.thread, model, result.historyComplete)
      if (historyEvents?.length) replayOpenCodeEventsAfterHistory(model, historyEvents, ref.id, {
        messageSnapshots: result.historyMessageSnapshots,
        statusAfterSequence: result.historyStatusSequence,
        authoritativeStatus: result.historyStatusSequence >= 0 ? result.thread?.status : null,
      })
    }
    if (state.backend === ref.backend && state.selectedId === ref.id) {
      markTranscriptHistoryReady({ complete: result.historyComplete !== false })
    } else {
      transcriptCaptureSuppressedKeys.delete(key)
    }
    mergeThreadIntoCatalog(ref.backend, result.thread)
    cacheThreadModel(ref.backend, ref.id, model, { historyEpoch })
    return model
  } finally {
    if (historyEvents) endOpenCodeHistoryEventBuffer(ref.id, historyEvents)
  }
}
return { scheduleOpenCodeLifecycleReconciliation, invalidateCodexBackendModelValidation, handleInactiveCodexAppServerStatus, handleCodexLifecycleLag, scheduleCodexLifecycleReconciliation, reconcileCodexLifecycleBackend, refreshOffscreenCodexHistoryAfterCompletion, handleCodexLifecycleNotification, handleOpenCodeServerEvent, ensureSessionModel }
}
