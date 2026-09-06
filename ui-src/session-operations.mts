import type { CodexViewModel, CodexTurn } from './codex-types.mjs'
import type { SessionCatalogEntry } from './backend-types.mjs'
import type { SessionModelOptions } from './session-model-preferences.mjs'
import { createCodexViewModel, hydrateCodexThread } from './codex-native.mjs'
import { threadForkParams } from './thread-fork.mjs'

export interface ManagedThread extends SessionCatalogEntry { turns?: CodexTurn[] | undefined; name?: string }
export interface ManagementElement {
  value: string; disabled: boolean; textContent: string | null; isConnected?: boolean
  classList: { add: (value: string) => void; remove: (value: string) => void }
  reset: () => void
}
export interface ManagementState {
  backend: string; selectedId: string | null; ready: boolean; hostPlatform: string
  model: CodexViewModel & { historyComplete?: boolean }
  threads: ManagedThread[]; threadsByBackend: Record<string, ManagedThread[]>
  backendModels: Record<string, { model?: string; id?: string }[]>
  turnOptions: Record<string, SessionModelOptions>
  selectedByBackend: Record<string, string | null>
  pinnedSessions: Set<string>
  annotationDrafts: Record<string, unknown>; annotationAdditional: Record<string, unknown>
  openingMessages: Record<string, unknown>; messageQueues: Record<string, unknown>
  pausedMessageQueues: Set<string>; messageQueueErrors: Map<string, string>
}
const messageOf = (error: unknown) => error instanceof Error ? error.message : String(error)
export interface ManagementServices {
  $: (selector: string) => ManagementElement
  t: (text: string, variables?: Record<string, string>) => string
  confirm: (text: string) => boolean
  rpc: (method: string, params: Record<string, unknown>) => Promise<{ thread?: ManagedThread }>
  selectedStateKey: (id?: string | null, backend?: string) => string
  selectedThread: () => ManagedThread | null
  currentBackend: () => { name: string }
  backendDescriptor: (backend: string) => { name: string }
  isCodexBackend: (backend: string) => boolean
  isArchivedPreview: () => boolean
  defaultTurnOptions: (backend: string) => SessionModelOptions
  switchBackend: (backend: string) => Promise<unknown>
  waitFor: (predicate: () => boolean, timeout: number) => Promise<unknown>
  rememberStartedThread: (backend: string, thread: ManagedThread, operation: string) => void
  forgetStartedThread: (backend: string, id: string, reason: string) => unknown
  hydrateOpenCodeModelMetadata: (thread: ManagedThread, model: CodexViewModel, complete: boolean) => void
  cacheThreadModel: (backend: string, id: string, model: CodexViewModel) => void
  sessionDispatch: { markPrepared: (ref: { backend: string; id: string }) => void }
  selectThread: (id: string, options: { force: boolean; backend: string }) => Promise<unknown>
  connectionReady: () => void
  freshThreadModel: (backend: string, id: string) => unknown
  reportSessionLifecycle: (phase: string, details: Record<string, unknown>) => void
  scheduleStartedThreadCatalogConfirmation: (backend: string, id: string) => void
  threadForRef: (ref: { backend: string; id: string }) => ManagedThread | null
  closeNewThreadDialog: () => void
  closeRenameThreadDialog: () => void
  persistSessionTurnOptions: (key: string) => Promise<unknown>
  persistSessionPin: (key: string, pinned: boolean) => Promise<unknown>
  deletePersistedSessionState: (key: string) => unknown
  discardComposerSessionState: (backend: string, id: string) => void
  invalidateThreadModel: (backend: string, id: string) => void
  sessionManagement: { archive: { markStale: () => void; remove: (backend: string, id: string) => void } }
  persistPreferences: () => unknown
  loadThreads: () => Promise<unknown>
  renderThreadList: () => void; renderWorkspace: () => void; renderTranscript: () => void
  toast: (message: string) => void; showError: (error: unknown) => void
}
export function createSessionOperations(state: ManagementState, services: ManagementServices) {
  const { $, t, confirm, rpc, selectedStateKey, selectedThread, currentBackend, backendDescriptor, isCodexBackend, isArchivedPreview, defaultTurnOptions, switchBackend, waitFor, rememberStartedThread, forgetStartedThread, hydrateOpenCodeModelMetadata, cacheThreadModel, sessionDispatch, selectThread, connectionReady, freshThreadModel, reportSessionLifecycle, scheduleStartedThreadCatalogConfirmation, threadForRef, closeNewThreadDialog, closeRenameThreadDialog, persistSessionTurnOptions, persistSessionPin, deletePersistedSessionState, discardComposerSessionState, invalidateThreadModel, sessionManagement, persistPreferences, loadThreads, renderThreadList, renderWorkspace, renderTranscript, toast, showError } = services
function mergeThreadIntoCatalog(backend: string, incoming: ManagedThread | null | undefined) {
  if (!incoming?.id) return
  const catalog = state.threadsByBackend[backend] || (state.threadsByBackend[backend] = [])
  const metadata = { ...incoming, turns: undefined }
  const index = catalog.findIndex((thread) => thread.id === incoming.id)
  if (index >= 0) catalog[index] = { ...catalog[index], ...metadata }
  else catalog.unshift(metadata)
  if (backend === state.backend) state.threads = catalog
}

function activateStartedThread(backend: string, thread: ManagedThread | undefined, { operation = 'new' } = {}) {
  if (!thread?.id) throw new Error(t('The backend created a session without an ID.'))
  if (backend !== state.backend || !state.ready) {
    throw new Error(t('The new session backend changed before Studio could open it.'))
  }

  rememberStartedThread(backend, thread, operation)
  const model: CodexViewModel & { historyComplete?: boolean } = createCodexViewModel()
  hydrateCodexThread(model, thread)
  if (backend === 'opencode') hydrateOpenCodeModelMetadata(thread, model, true)
  model.historyComplete = true
  cacheThreadModel(backend, thread.id, model)
  // thread/start and thread/fork already leave the returned thread active in
  // this App Server process. A redundant thread/resume can fail for a blank
  // thread that has not reached the state database yet.
  sessionDispatch.markPrepared({ backend, id: thread.id })
  renderThreadList()

  const selection = selectThread(thread.id, { force: true, backend })
  connectionReady()
  const selected = state.selectedId === thread.id && selectedThread()?.id === thread.id
  reportSessionLifecycle('selected', {
    backend,
    threadId: thread.id,
    operation,
    selected,
    cached: Boolean(freshThreadModel(backend, thread.id)),
  })
  if (!selected) {
    return Promise.reject(new Error(t('Studio created the session but could not select it.')))
  }
  scheduleStartedThreadCatalogConfirmation(backend, thread.id)
  return selection.then(() => {
    reportSessionLifecycle('ready', {
      backend,
      threadId: thread.id,
      operation,
      selected: state.backend === backend && state.selectedId === thread.id,
      catalogContains: Boolean(threadForRef({ backend, id: thread.id! })),
    })
    return thread.id
  }).catch((error) => {
    reportSessionLifecycle('activation-failed', {
      backend,
      threadId: thread.id,
      operation,
      error: messageOf(error),
    })
    throw error
  })
}

async function renameSelectedThread(event: { preventDefault(): void }) {
  event.preventDefault()
  const threadId = state.selectedId
  const name = $('#rename-thread-name').value.trim()
  const errorBox = $('#rename-thread-error')
  if (!threadId || !name) return
  errorBox.classList.add('hidden')
  try {
    await rpc('thread/name/set', { threadId, name })
    const thread = state.threads.find((candidate) => candidate.id === threadId)
    if (thread) thread.name = name
    closeRenameThreadDialog()
    renderThreadList()
    renderWorkspace()
    toast('Session name saved')
  } catch (error) {
    errorBox.textContent = messageOf(error)
    errorBox.classList.remove('hidden')
  }
}

async function createThread(event: { preventDefault(): void }) {
  event.preventDefault()
  const button = $('#create-thread')
  const errorBox = $('#new-thread-error')
  let createdThreadId = ''
  button.disabled = true
  errorBox.classList.add('hidden')
  const backend = $('#new-thread-backend').value
  const name = $('#new-thread-name').value.trim()
  const cwd = $('#new-thread-cwd').value.trim()
  if (state.hostPlatform === 'windows' && !cwd.startsWith('/')) {
    errorBox.textContent = t('The Windows client requires an absolute Linux path inside WSL.')
    errorBox.classList.remove('hidden')
    button.disabled = false
    return
  }
  const params: Record<string, unknown> = {
    cwd,
    ...(isCodexBackend(backend) ? {
      approvalPolicy: $('#new-thread-approval').value,
      sandbox: $('#new-thread-sandbox').value,
    } : {}),
  }
  const model = $('#new-thread-model').value.trim()
  const supportedModels = state.backendModels[backend]!.map((entry) => entry.model || entry.id).filter(Boolean)
  if (model && supportedModels.length && !supportedModels.includes(model)) {
    errorBox.textContent = `${model} is not in the current ${backendDescriptor(backend).name} model catalog.`
    errorBox.classList.remove('hidden')
    button.disabled = false
    return
  }
  if (model) params.model = model
  try {
    if (backend !== state.backend) {
      await switchBackend(backend)
      await waitFor(() => state.backend === backend && state.ready, 15_000)
    }
    const result = await rpc('thread/start', params)
    if (!result?.thread?.id) throw new Error(t('The backend created a session without an ID.'))
    createdThreadId = result.thread.id
    const startedThread = name ? { ...result.thread, name } : result.thread
    if (model) {
      const key = selectedStateKey(createdThreadId, backend)
      state.turnOptions[key] = { ...defaultTurnOptions(backend), model }
    }
    const activation = activateStartedThread(backend, startedThread)
    closeNewThreadDialog()
    $('#new-thread-form').reset()
    await Promise.all([
      activation,
      model ? persistSessionTurnOptions(selectedStateKey(createdThreadId, backend)) : null,
      name ? rpc('thread/name/set', { threadId: createdThreadId, name }) : null,
    ])
    toast(t('{backend} session created', { backend: backendDescriptor(backend).name }))
  } catch (error) {
    reportSessionLifecycle('create-failed', {
      backend,
      threadId: createdThreadId,
      error: messageOf(error),
    })
    if (createdThreadId) showError(error)
    else {
      errorBox.textContent = messageOf(error)
      errorBox.classList.remove('hidden')
    }
  } finally { button.disabled = false }
}

async function forkThread(lastTurnId: string | null = null, trigger: ManagementElement | null = null) {
  const sourceThreadId = state.selectedId
  const sourceBackend = state.backend
  const sourceOptions = { ...(state.turnOptions[selectedStateKey(sourceThreadId, sourceBackend)] || {}) }
  if (!sourceThreadId || isArchivedPreview()) return
  if (trigger) {
    trigger.disabled = true
    trigger.classList.add('busy')
  }
  try {
    const result = await rpc('thread/fork', threadForkParams(sourceThreadId, lastTurnId))
    if (!result?.thread?.id) throw new Error(t('The backend created a session without an ID.'))
    const forkKey = selectedStateKey(result.thread.id, sourceBackend)
    if (sourceOptions.model || sourceOptions.effort) {
      state.turnOptions[forkKey] = sourceOptions
    }
    await Promise.all([
      activateStartedThread(sourceBackend, result.thread, { operation: lastTurnId ? 'fork-turn' : 'fork' }),
      sourceOptions.model || sourceOptions.effort ? persistSessionTurnOptions(forkKey) : null,
    ])
    toast(t(lastTurnId ? 'Created a {backend} session fork from this turn' : '{backend} session fork created', { backend: currentBackend().name }))
  } catch (error) {
    showError(error)
    if (trigger?.isConnected) {
      trigger.disabled = false
      trigger.classList.remove('busy')
    }
  }
}

async function archiveSelectedThread() {
  if (!state.selectedId || !confirm(t('Archive the current Codex session?'))) return
  const threadId = state.selectedId
  const key = selectedStateKey(threadId, state.backend)
  try {
    await rpc('thread/archive', { threadId })
    forgetStartedThread(state.backend, threadId, 'archived')
    if (state.pinnedSessions.delete(key)) {
      persistSessionPin(key, false).catch(showError)
    }
    discardComposerSessionState(state.backend, threadId)
    invalidateThreadModel(state.backend, threadId)
    state.selectedId = null
    state.selectedByBackend[state.backend] = null
    state.model = createCodexViewModel()
    sessionManagement.archive.markStale()
    persistPreferences()
    await loadThreads()
    toast('Session archived')
  } catch (error) { showError(error) }
}

async function deleteSelectedThread() {
  if (!state.selectedId || !confirm(t('Permanently delete this Codex session and its stored history? This cannot be undone.'))) return
  const threadId = state.selectedId
  const archived = isArchivedPreview()
  try {
    await rpc('thread/delete', { threadId })
    forgetStartedThread(state.backend, threadId, 'deleted')
    invalidateThreadModel(state.backend, threadId)
    const deletedKey = `${state.backend}:${threadId}`
    state.pinnedSessions.delete(deletedKey)
    delete state.annotationDrafts[deletedKey]
    delete state.annotationAdditional[deletedKey]
    delete state.openingMessages[deletedKey]
    delete state.turnOptions[deletedKey]
    delete state.messageQueues[deletedKey]
    state.pausedMessageQueues.delete(deletedKey)
    state.messageQueueErrors.delete(deletedKey)
    deletePersistedSessionState(deletedKey)
    discardComposerSessionState(state.backend, threadId)
    state.selectedId = null
    if (!archived) state.selectedByBackend[state.backend] = null
    state.model = createCodexViewModel()
    if (archived) {
      sessionManagement.archive.remove(state.backend, threadId)
    }
    persistPreferences()
    if (archived) {
      renderThreadList()
      renderWorkspace()
      renderTranscript()
    } else await loadThreads()
    toast('Session deleted')
  } catch (error) { showError(error) }
}
return { mergeThreadIntoCatalog, activateStartedThread, renameSelectedThread, createThread, forkThread, archiveSelectedThread, deleteSelectedThread }
}
