import { createComposerActions } from './composer-actions.mjs'
import { createSettingsApplication } from './settings-application.mjs'
import { createActiveCodexConnection } from './active-codex-connection.mjs'
import { createOpenCodeProtocol } from './opencode-protocol.mjs'
import { createSubmissionController } from './submission-controller.mjs'
import { createBackgroundSessions } from './background-sessions.mjs'
import { createSessionOperations } from './session-operations.mjs'
import { preferencesSnapshot as createPreferencesSnapshot } from './preferences-snapshot.mjs'
import { createEnvironmentApplication } from './environment-application.mjs'
import { installSelectedHistory, hydrateOpenCodeHistoryMetadata } from './history-installation.mjs'
import { awaitBackendSelection, completeSessionSelection, SessionWorkspaceMemory } from './selection-coordinator.mjs'
import { createStartedSessionCatalog } from './started-session-catalog.mjs'
import { runHiddenUtilitySession } from './hidden-utility-session.mjs'
import { createTurnSupervision, SUPERVISION_SCHEMA, SUPERVISION_INSTRUCTIONS, supervisionResult } from './turn-supervision.mjs'
import { createSessionStatePersistence } from './session-state-persistence.mjs'
import { connectLifecycleStream, connectEventStream, waitForEventStream } from './lifecycle-connection.mjs'
import {
  applyCodexNotification,
  createCodexViewModel,
  hydrateCodexThread,
  resolveCodexApproval,
  resolveCodexInteraction,
  textFromUserContent,
} from './codex-native.mjs'
import { createOpenCodeLoopGuard, mergeOpenCodeThreadTail, normalizeOpenCodeStatus, replayOpenCodeEventsAfterHistory } from './opencode-native.mjs'
import { resolveModelDisplay } from './model-display.mjs'
import { createTranscriptDom } from './transcript-dom.mjs'
import {
  catalogListParams,
  mergeCatalogMetadata,
  shouldRecoverCodexCatalog,
  turnStartParams,
} from './session-catalog.mjs'
import {
  createSessionLibraryState,
  createSessionManagementUI,
  createThreadSearchState,
} from './session-management.mjs'
import {
  BACKEND_IDS,
  backendDescriptor,
  backendDescriptors,
  defaultTurnOptions,
  emptyBackendCatalogs,
  emptyBackendSelections,
  installBackendRegistry,
  isCodexBackend,
  isSupportedBackend,
} from './backends.mjs'
import { composerReferenceInput, pluginReferences, composerTrigger, createComposerDraftStore, fuzzyFileLabel, matchingSkills, matchingSlashCommands, replaceComposerTrigger, reviewableFileKind, selectedFileReference, selectedSkillReference, shellCommandFromComposer, transcriptUpdateKind } from './composer-tools.mjs'
import { MAX_COMPOSER_IMAGES, MAX_COMPOSER_IMAGE_TOTAL_BYTES, formatImageSize, prepareComposerImage, userImagesFromContent } from './composer-images.mjs'
import {
  resolveMarkdownFileLink,
  resolveMarkdownImagePath,
} from './document-review.mjs'
import {
  extractMarkdownOutline,
} from './document-outline.mjs'
import {
  CommentSourceRegistry,
  normalizeCommentDrafts,
} from './comment-core.mjs'
import {
  createChatCommentProvider,
  createDocumentCommentProvider,
  legacyCommentSource,
} from './comment-source-providers.mjs'
import { browserCommentSource, createBrowserCommentProvider } from './browser-comment-provider.mjs'
import { createEpubCommentProvider } from './epub-comment-provider.mjs'
import { createPdfCommentProvider } from './pdf-comment-provider.mjs'
import { createTableCommentProvider } from './table-comment-provider.mjs'
import { questionForTurn } from './favorites.mjs'
import { MERMAID_PREFERENCES_DEFAULTS, mermaidInitializeConfig } from './mermaid-config.mjs'
import {
  sessionMapKey,
  sessionMapVisibleText,
} from './session-map.mjs'
import {
  createSessionMapController,
  createSessionMapRuntimeState,
} from './session-map-controller.mjs'
import {
  createReviewNotesController,
  createReviewNotesState,
} from './review-notes-controller.mjs'
import {
  createDocumentWorkspaceController,
  createDocumentWorkspaceState,
} from './document-workspace-controller.mjs'
import { marked } from './vendor/marked.esm.js'
import {
  activeTurnAtMarker,
  navigableTurns,
  turnPromptPreview,
} from './turn-navigator.mjs'
import {
  TranscriptPresentationCache,
  activityOutputPreview,
  presentationActivityBlocks,
  presentationActivityEntries,
  presentRoutedTurn,
  reasoningStage,
  shouldShowTurnPlaceholder,
} from './transcript-presentation.mjs'
import { isTurnForkable } from './thread-fork.mjs'
import {
  SELECTION_TRANSLATION_INSTRUCTIONS,
  SELECTION_TRANSLATION_SCHEMA,
  selectionTranslationInput,
  translationCacheKey,
  translationTurnState,
} from './selection-translation.mjs'
import { PendingRpcRequests, requestSocketRpc, connectSelectedSocket } from './rpc-lifecycle.mjs'
import { normalizeTypography as normalizeTypographyProfile } from './preference-normalization.mjs'

import {
  annotationPromptDefaults,
  getLocale,
  normalizeLocalizedTemplates,
  resolveLanguage,
  setLanguage,
  startTranslationObserver,
  t,
} from './i18n.mjs'
import {
  createTranscriptContentObserver,
  createTranscriptScrollFollower,
  distanceFromBottom,
  shouldFollowLatestOnReturn,
  shouldPinTranscriptOnTakeover,
  transcriptResizeAction,
  transcriptScrollEventAction,
  transcriptRestorePlan,
} from './transcript-scroll.mjs'
import { createWorkspaceTools } from './workspace-tools.mjs'
import { createSessionResourcesUI } from './session-resources-ui.mjs'
import { rightRailWidthBounds } from './right-rail-layout.mjs'
import { catalogActivityTimestamp, catalogCountsWithAttention, catalogTimestamp, compactSidebarText, filterCatalogEntries, groupCatalogEntries, isCatalogCacheFresh, isSessionDirectoryHidden, partitionPinnedCatalogEntries, syncCatalogSelection, threadCatalogKey } from './thread-catalog.mjs'
import {
  addLoadedThread,
  preserveCatalogActivity,
  restoreCatalogThreadActivity,
  updateCatalogThreadActivity,
} from './thread-workset.mjs'
import { catalogsWithSingleRouter, finalAgentText, isRouterSession, managedRouterThread, normalizeThreadRouter, parseSessionRefKey, recoverManagedRouterCatalog, sessionRefKey } from './thread-router.mjs'
import {
  createThreadRouterController,
  createThreadRouterRuntimeState,
  routerRuntimeKey,
} from './thread-router-controller.mjs'
import { SessionDispatchRegistry } from './session-dispatch.mjs'
import { createCodexHistoryLoader } from './codex-history-loader.mjs'
import { coordinateHistoryLoad } from './history-load-coordinator.mjs'
import { createSerializedStateWriter } from './serialized-state-writer.mjs'
import { copySessionTurnOptions } from './session-model-preferences.mjs'
import { cachedSession, storeCachedSession, validateCachedModel, unvalidateCachedModel, cachedModelThreadId, routeCodexNotification } from './session-model-cache.mjs'
import DOMPurify from './vendor/purify.es.mjs'
import { formatEnvironmentLines, environmentSavePayload } from './environment-profile.mjs'
import { createPerformanceMonitor, exposePerformanceMonitor } from './performance-monitor.mjs'
import {
  codexLifecycleEvent,
  claimLifecycleNotification,
} from './codex-lifecycle-diagnostics.mjs'
import {
  DEFAULT_TURN_TAIL_PAGE_SIZE,
  isHistoryPaginationCompatibilityError,
} from './thread-history-tail.mjs'

const studioPerformance = createPerformanceMonitor()
exposePerformanceMonitor(studioPerformance)

const commentSources = new CommentSourceRegistry()
  .register(createChatCommentProvider())
  .register(createDocumentCommentProvider())
  .register(createEpubCommentProvider())
  .register(createPdfCommentProvider())
  .register(createTableCommentProvider())
  .register(createBrowserCommentProvider())

function codexDispatchAdapter() {
  return {
    read: (ref) => dispatchBackendRpc(ref.backend, 'thread/read', { threadId: ref.id, includeTurns: true }),
    prepareTurn: (ref) => prepareCodexThreadWithoutHistory(ref),
    startTurn: (ref, input, options = {}) => dispatchBackendRpc(ref.backend, 'turn/start', turnStartParams('codex', threadForRef(ref), {
      threadId: ref.id,
      clientUserMessageId: options.clientUserMessageId || randomId(),
      input,
      ...(options.additionalContext ? { additionalContext: options.additionalContext } : {}),
      ...(options.outputSchema ? { outputSchema: options.outputSchema } : {}),
      ...(options.turnOptions || {}),
    }), options.timeoutMs),
  }
}

const sessionDispatch = new SessionDispatchRegistry()
  .register('codex', codexDispatchAdapter())
  .register('opencode', {
    read: (ref) => dispatchBackendRpc(ref.backend, 'thread/read', { threadId: ref.id, includeTurns: true }),
    startTurn: (ref, input, options = {}) => {
      return dispatchBackendRpc(ref.backend, 'turn/start', {
        threadId: ref.id,
        input,
        ...(options.developerInstructions ? { developerInstructions: options.developerInstructions } : {}),
        ...(options.outputSchema ? { outputSchema: options.outputSchema } : {}),
        ...(options.turnOptions || {}),
      }, options.timeoutMs)
    },
  })

const codexHistoryLoader = createCodexHistoryLoader({
  dispatchBackendRpc, requestCodexResume, historyTailCapability,
  rememberHistoryTailCapability, threadForRef,
})

marked.setOptions({
  async: false,
  breaks: true,
  gfm: true,
})

const $ = (selector) => document.querySelector(selector)
const $$ = (selector) => [...document.querySelectorAll(selector)]

const defaultUiFontFamily = '"Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif'
const legacyDefaultUiFontFamilies = new Set([
  'Ubuntu, "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif',
  'Inter, "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif',
])

const typographyDefaults = Object.freeze({
  uiFontFamily: defaultUiFontFamily,
  uiFontSize: 14,
  uiFontWeight: 500,
  contentFontFamily: defaultUiFontFamily,
  contentFontSize: 15,
  contentFontWeight: 500,
  codeFontFamily: '"JetBrains Mono", "SFMono-Regular", Consolas, monospace',
  codeFontSize: 14,
  codeFontWeight: 500,
  highContrast: true,
})

function defaultAnnotationPrompt(locale = getLocale()) {
  return annotationPromptDefaults[locale] || annotationPromptDefaults['en-US']
}

const state = {
  backend: 'codex',
  selectedByBackend: emptyBackendSelections(),
  startupRouterSelectionPending: true,
  socket: null,
  socketGeneration: 0,
  reconnectTimer: null,
  ready: false,
  backendInfo: null,
  backendInfos: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null])),
  backendModels: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, []])),
  backendRegistry: { configPath: '', configurationError: null },
  backendModelLoads: new Map(),
  hostPlatform: window.__CODEX_THREAD_STUDIO_GATEWAY__?.hostPlatform || null,
  remoteClient: Boolean(window.__CODEX_THREAD_STUDIO_GATEWAY__?.remote),
  wsl: { distribution: '', user: '', codexBinary: 'codex', opencodeBinary: 'opencode' },
  backendStates: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, backend === 'codex'
    ? { kind: 'checking', label: 'Starting Codex', caption: 'App Server · stdio' }
    : { kind: 'idle', label: backendDescriptor(backend).name, caption: 'Connect on demand' }])),
  requestId: 0,
  pending: new Map(),
  threads: [],
  threadsByBackend: emptyBackendCatalogs(),
  threadModels: new Map(),
  threadLoads: new Map(),
  historyTailCapabilities: new Map(),
  selectedId: null,
  search: '',
  filter: 'all',
  sessionLibrary: createSessionLibraryState(),
  threadSearch: createThreadSearchState(),
  attentionThreads: new Set(),
  pinnedSessions: new Set(),
  collapsedThreadGroups: new Set(),
  model: createCodexViewModel(),
  language: 'system',
  theme: 'light',
  contentWidth: 'comfortable',
  hiddenSessionDirectories: [],
  sharedDocumentDirectories: [],
  sessionDirectoryIgnore: [],
  sidebarCollapsed: false,
  rightRailWidthRatio: 0.44,
  typography: { ...typographyDefaults },
  mermaid: { ...MERMAID_PREFERENCES_DEFAULTS },
  markdown: { mode: 'technical' },
  translation: { engine: 'backend', ollamaModel: 'gemma3:4b', models: {}, efforts: {} },
  ollamaModels: [],
  desktopNotifications: false,
  queueDepth: 1,
  continueBehavior: 'sessionModelDraft',
  appServerCapabilities: {},
  appServerInitialization: null,
  appServerGenerations: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null])),
  environmentProfile: null,
  environmentProfileSelectionRoot: '',
  browser: null,
  browserInfo: null,
  embeddedBrowserVisible: false,
  embeddedBrowserLoaded: false,
  activeRightWorkspace: null,
  ...createReviewNotesState(),
  annotationPromptTemplates: {},
  activeAnnotationPromptTemplate: annotationPromptDefaults['en-US'],
  openingMessages: {},
  ...createDocumentWorkspaceState(),
  composerMenu: { type: null, trigger: null, options: [], selected: 0, generation: 0 },
  skillCatalog: { cwd: null, skills: [], request: null, loaded: false },
  turnOptions: {},
  messageQueues: {},
  pausedMessageQueues: new Set(),
  runningMessageQueues: new Set(),
  messageQueueErrors: new Map(),
  pendingSkills: {},
  pendingFiles: {},
  pendingImages: {},
  ...createSessionMapRuntimeState(),
  hiddenCodexThreads: new Set(),
  hiddenCodexTurns: new Set(),
  hiddenUtilityThreads: new Set(),
  hiddenUtilityThreadNames: new Set(),
  selectionTranslationCache: new Map(),
  structuredUtilityTasks: new Map(),
  continuationDraftLoads: new Set(),
  router: normalizeThreadRouter(null),
  routerRuntime: createThreadRouterRuntimeState(),
}

let preferencesReady = false
const preferencesWriter = createSerializedStateWriter(gatewayFetch, (error) => console.error('Unable to persist preferences', error))
const sessionStateWriter = createSerializedStateWriter(gatewayFetch, (error) => console.error('Unable to persist session state', error))
const sessionPersistence = createSessionStatePersistence(state, sessionStateWriter, () => preferencesReady)
let preferencesPersistTimer = null
let transcriptFrame = null
const dirtyStreamItems = new Map()
const turnLatencyTraces = new Map()
let composerSearchTimer = null
let codexCatalogFocusRefreshAt = 0
let artifactMarkdownImageObserver = null
let turnNavigatorFrame = null
let turnNavigatorRenderFrame = null
let turnNavigatorRenderIdle = null
let turnNavigatorRenderTimer = null
let turnNavigatorRenderGeneration = 0
let turnNavigatorRenderKey = ''
let turnNavigatorRenderedKey = ''
let activeTurnNavigatorButton = null
let turnNavigatorButtons = new Map()
let turnNavigatorIds = new Set()
let turnNavigatorSignature = ''
let transcriptCaptureFrame = null
const transcriptDom = createTranscriptDom()
const transcriptItemNodes = new Map()
let transcriptCaptureFrameKey = ''
let transcriptLiveLayoutAnchor = null
let transcriptUserScrollIntentUntil = 0
let transcriptPointerScrollActive = false
let openCodeListRefreshTimer = null
let openCodeListRefreshNeedsHistory = false
const openCodeStreamState = { stream: null, ready: false, missedBarrier: false, waiters: new Set(), openCount: 0 }
let openCodeHistoryEpoch = 0
const openCodeStatusReconcileTimers = new Map()
const openCodeLoopAbortRequests = new Set()
const openCodeLoopGuard = createOpenCodeLoopGuard()
const openCodeHistoryEventBuffers = new Map()
const openCodeHistoryEventSequences = new Map()
const transcriptCaptureSuppressedKeys = new Set()
let threadCatalogRetryTimer = null
let threadCatalogRetryAttempt = 0
let threadCatalogErrorMessage = null
const catalogRefreshes = new Map()
const catalogRequestGenerations = new Map()
const backendSelectionLoads = new Map()
const startedSessionCatalog = createStartedSessionCatalog({
  generations: catalogRequestGenerations,
  key: threadCatalogKey,
  catalog: backend => state.threadsByBackend[backend] || [],
  merge: mergeThreadIntoCatalog,
  report: reportSessionLifecycle,
  refresh: backend => refreshBackendCatalog(backend, { includeStatuses: false }),
  refreshError: (backend, error) => console.debug(`Unable to confirm newly started ${backend} session in the catalog`, error),
  missingId: () => t('The backend created a session without an ID.'),
})
const codexCatalogRecoveryStarted = new Set()
let inactiveCatalogRefreshScheduled = false
const codexLifecycleConnection = { socket: null, reconnectTimer: null, generation: 0, ready: false, openCount: 0 }
const handledCodexTurnLifecycleEvents = new Map()
const recentCodexStatusLifecycleEvents = new Map()
const codexBackendsNeedingRestartRecovery = new Set()
const transcriptScrollFollower = createTranscriptScrollFollower()
const transcriptContentObserver = createTranscriptContentObserver({ onResize: handleTranscriptContentResize })
const composerDrafts = createComposerDraftStore()
const transcriptPresentationCache = new TranscriptPresentationCache({ visibleTurns: 30 })
const markdownRenderCache = new Map()
const MAX_MARKDOWN_RENDER_CACHE_BYTES = 24 * 1024 * 1024
let markdownRenderCacheBytes = 0
const MAX_MERMAID_SOURCE_CHARS = 100_000
let mermaidObserver = null
let mermaidRenderChain = Promise.resolve()
let mermaidRenderSequence = 0
let mermaidGeneration = 0
let mermaidInitializedConfig = ''
let activityLogContext = null
let rightRailResize = null
let embeddedBrowserWidthTimer = null
let environmentDialogRoot = ''
let environmentDialogProfile = null
const environmentSecretRemovals = new Set()
const appliedEnvironmentProfiles = new Set()
const environmentApplyRequests = new Map()
const environmentApplication = createEnvironmentApplication({
  applied: appliedEnvironmentProfiles,
  requests: environmentApplyRequests,
  supports: isCodexBackend,
  apply: async params => {
    const response = await gatewayFetch('/studio/environment/apply', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(params),
    })
    const result = await response.json().catch(() => null)
    if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
    return result
  },
  compatibilityError: isHistoryPaginationCompatibilityError,
  disableTail: (backend, threadId) => rememberHistoryTailCapability(backend, threadId, false),
})
let pendingTranscriptViewRestore = null

const workspaceTools = createWorkspaceTools({
  gatewayFetch,
  gatewayWebSocket,
  getThread: selectedThread,
  getBackend: () => state.backend,
  getSharedDocumentDirectories: () => state.sharedDocumentDirectories,
  openFile: (file, context = {}) => openArtifact(file, { returnTool: context.returnTool }),
  canOpenFile: (file) => Boolean(reviewableFileKind(file)),
  closePeerRails: closeWorkspacePeerRails,
  translate: t,
  notify: toast,
})

const sessionResources = createSessionResourcesUI({
  getModel: () => state.model,
  getThread: selectedThread,
  getBackend: () => state.backend,
  activate: activateRightWorkspace,
  deactivate: (tool) => {
    deactivateRightWorkspace(tool)
    sessionMap.render()
  },
  openResource: openSessionResource,
  openSource: openSessionResourceSource,
  favoriteResource: (resource, occurrence) => reviewNotes.openFavoriteForResource(resource, occurrence),
  translate: t,
  notify: toast,
  performanceMonitor: studioPerformance,
})

const sessionManagement = createSessionManagementUI({
  state,
  backend: {
    requestCodexBackend,
    rpc,
    switchBackend,
    waitFor,
  },
  catalog: {
    selectThread,
    loadThreads,
    installBackendCatalog,
    selectedThread,
    threadTitle,
  },
  view: {
    createViewModel: createCodexViewModel,
    hydrateThread: hydrateCodexThread,
    captureTranscriptView: captureTranscriptViewState,
    prepareTranscriptView: prepareTranscriptViewForSelection,
    closeActionMenus,
    closeWorkspacePeerRails,
    renderThreadList,
    renderWorkspace,
    renderTranscript,
  },
  searchView: {
    showTurn: (turnId) => transcriptPresentationCache.showTurn(presentationThreadKey(), state.model, turnId),
    renderedItem,
    activityBlocks: activityBlocksForTurn,
    renderedActivity,
    hydrateActivity: hydrateActivityDetails,
    pauseFollowing: beginTranscriptProgrammaticNavigation,
  },
  notify: toast,
  reportError: showError,
})

const threadRouter = createThreadRouterController({
  state,
  gatewayFetch,
  dispatch: sessionDispatch,
  backend: {
    dispatchRpc: dispatchBackendRpc,
    refreshCatalogs: refreshRouterCatalogs,
    loadBackendInfo,
    configuredTurnOptions,
    targetTurnOptions: ref => {
      const options = sessionTurnOptions(ref)
      const model = options.model || threadForRef(ref)?.model
      return { ...options, ...(model ? { model } : {}) }
    },
  },
  catalog: {
    sidebarCatalogs: sidebarThreadCatalogs,
    threadTitle,
    threadStatus,
    mergeThread: mergeThreadIntoCatalog,
    updateLoadedThreadTimestamp,
  },
  model: {
    ensureSessionModel,
    applyNotification: applyCodexNotification,
    cacheThreadModel,
  },
  view: {
    closeActionMenus,
    renderThreadList,
    renderWorkspace,
    renderTranscript,
    renderComposerState,
    refreshSupervision: renderComposerTools,
    refreshTurnNavigator: scheduleTurnNavigatorRender,
    preserveAttentionLayout: preserveTranscriptLayout,
    captureReadingPosition: captureRouterReadingPosition,
    restoreReadingPosition: position => {
      showRouterResponse(position.turnId, false)
      const transcript = $('#transcript')
      const anchor = transcript.querySelector(`.turn[data-turn-id="${CSS.escape(position.turnId)}"]`)
      transcript.scrollTop = anchor ? transcript.scrollTop + anchor.getBoundingClientRect().top - transcript.getBoundingClientRect().top - position.offset : position.scrollTop
      captureTranscriptViewState()
    },
    showRouterResponse,
    revealTargetTurn: navigateTranscriptTurn,
    renderSupervisionAction: renderSupervisionMenu,
    openTargetPicker: options => {
      if (state.composerMenu.type === 'router' && state.composerMenu.trigger?.confirmedPicker) { hideComposerMenu(); return }
      const cursor = $('#composer-input').selectionStart
      state.composerMenu = { ...state.composerMenu, type: 'router', options: [{ kind: 'session', automatic: true, key: '', title: t('Automatic routing') }, ...options.map(option => ({ ...option, kind: 'session' }))], selected: 0,
        trigger: { start: cursor, end: cursor, query: '', confirmedPicker: true }, fileMessage: '', generation: state.composerMenu.generation + 1 }
      state.composerMenu.selected = Math.max(0, state.composerMenu.options.findIndex(option => option.key === state.routerRuntime.selectedTarget))
      $('#router-choose-target').setAttribute('aria-expanded', 'true')
      renderComposerMenu()
      $('#composer-input').focus()
    },
    renderItem,
    renderTargetTurn: renderRouterTargetTurn,
    selectThread,
    setComposerValue: setCurrentComposerValue,
    hideComposerMenu,
    openWorkspaceTool: (source, tool) => workspaceTools.openForSession(source.thread, source.backend, tool),
  },
  persistPreferences,
  notify: toast,
})

const sessionMap = createSessionMapController({
  state,
  transport: {
    gatewayFetch,
    gatewayWebSocket,
    rpc,
    dispatchBackendRpc,
    sendRaw,
  },
  model: {
    createViewModel: createCodexViewModel,
    applyNotification: applyCodexNotification,
  },
  view: {
    selectedStateKey,
    activateRightWorkspace,
    deactivateRightWorkspace,
    closeActionMenus,
    toggleActionMenu,
    closeAnnotationRail: () => reviewNotes.closeAnnotations(),
    closeFavoritesRail: () => reviewNotes.closeFavorites(),
    renderArtifact,
    isResourcesOpen: () => sessionResources.isOpen(),
    isWorkspaceOpen: () => workspaceTools.isOpen(),
  },
  randomId,
  notify: toast,
  reportError: showError,
})

const pendingRpcRequests = new PendingRpcRequests(state.pending)
const sessionOperations = createSessionOperations(state, {
  $, t, confirm, rpc, selectedStateKey, selectedThread, currentBackend, backendDescriptor,
  isCodexBackend, isArchivedPreview, defaultTurnOptions, switchBackend, waitFor,
  rememberStartedThread, forgetStartedThread, hydrateOpenCodeModelMetadata, cacheThreadModel,
  sessionDispatch, selectThread, freshThreadModel, reportSessionLifecycle,
  scheduleStartedThreadCatalogConfirmation, threadForRef, closeNewThreadDialog, closeRenameThreadDialog,
  persistSessionTurnOptions, persistSessionPin, deletePersistedSessionState, discardComposerSessionState,
  invalidateThreadModel, sessionManagement, persistPreferences, loadThreads,
  renderThreadList, renderWorkspace, renderTranscript, toast, showError,
  connectionReady: () => { $('#native-connection').textContent = t('Connected') },
})
const submissionController = createSubmissionController(state, {
  isSupervised: ref => turnSupervision.activeFor(ref),
  $, selectedStateKey, shellCommandFromComposer, matchingSlashCommands, executeSlashCommand,
  isRouterThread, threadRouter, rpc, dispatchBackendRpc, sessionDispatch, prepareComposerTurn,
  isCodexBackend, isSupportedBackend, backendDescriptor, currentBackend, threadForRef,
  configuredTurnOptions, queuedTurnOptions, messageQueueModel, setComposerDraftValue, composerDrafts,
  hideComposerMenu, renderComposerState, renderTranscript, beginTranscriptFollowingLatest,
  setCatalogThreadActivity, rollbackCatalogThreadActivity, markCachedModelValidated,
  beginTurnLatencyTrace, bindTurnLatencyTrace, markTurnLatency, finishTurnLatencyTrace,
  randomId, persistMessageQueue, showError, toast, t,
})

const reviewNotes = createReviewNotesController({
  state,
  commentSources,
  gatewayFetch,
  randomId,
  persistAnnotationState,
  notify: toast,
  reportError: showError,
  view: {
    selectedStateKey,
    selectedThread,
    threadTitle,
    closeActionMenus,
    activateRightWorkspace,
    syncRightWorkspaceLaunchers,
    renderArtifact,
    renderSessionMap: () => sessionMap.render(),
    openArtifact,
    setArtifactView,
    reopenEpubSource: reopenEpubComment,
    goToPdfPage: (page) => documentWorkspace.goToPdfPage(page),
    openBrowserUrl,
    renderMarkdown,
    renderedItem,
    setComposerValue: setCurrentComposerValue,
    submitComposer: () => sendComposer({ preventDefault() {} }),
    pauseTranscript: beginTranscriptProgrammaticNavigation,
    preserveTranscriptLayout,
    switchBackend,
    waitForBackend: waitFor,
    loadThreads,
    selectThread,
    translateSelection: translateSelectionWithCurrentBackend,
    translationProfile: currentSelectionTranslationProfile,
    sourceContext: (node) => threadRouter.sourceContext(node),
  },
})

const documentWorkspace = createDocumentWorkspaceController({
  state,
  gatewayFetch,
  randomId,
  notify: toast,
  reportError: showError,
  view: {
    selectedThread,
    selectedStateKey,
    activateRightWorkspace,
    hideComposerMenu,
    closeActionMenus,
    applyRightRailWidth,
    renderMarkdownDocument,
    hydrateMarkdownImages,
    disconnectMarkdownImageObserver,
    disposeMarkdownImageAssets,
    hideSelection: () => reviewNotes.hideSelection(),
    setSelection: (...args) => reviewNotes.setSelection(...args),
    renderDocumentCommentMarkers: () => reviewNotes.renderDocumentCommentMarkers(),
    openWorkspaceTool: (tool, sourceSessionKey) => {
      const ref = parseSessionRefKey(sourceSessionKey)
      if (!ref) return workspaceTools.open(tool)
      const thread = state.threadsByBackend[ref.backend]?.find(candidate => candidate.id === ref.id)
      if (!thread) throw new Error(t('The target session no longer exists.'))
      return workspaceTools.openForSession(thread, ref.backend, tool)
    },
    openResources: () => sessionResources.open(),
    renderSessionMap: () => sessionMap.render(),
    openBrowserUrl,
    readingTypography: () => ({
      fontFamily: state.typography.contentFontFamily,
      fontSize: state.typography.contentFontSize,
      fontWeight: state.typography.contentFontWeight,
    }),
    reportClientError,
  },
})

const openCodeProtocol = createOpenCodeProtocol(state, {
  gatewayFetch, selectedThread, currentTurnOptions, t, studioPerformance,
  threadCatalogKey, openCodeHistoryEventSequences,
})

const backgroundSessions = createBackgroundSessions(state, {
  $, codexBackendsNeedingRestartRecovery, scheduleStudioIdleWork, refreshBackendCatalog, refreshSelectedThread, threadStatus, handleQueuedTurnCompletion, backendDescriptor, sessionDispatch, clearStartedThreadsForBackend, isCodexBackend, renderConnectionsDialog, renderBackendDialog, reportSessionLifecycle, markCachedModelUnvalidated, markCachedModelValidated, threadCatalogKey, loadCodexHistoryForBackground, cachedThreadModel, mergeThreadIntoCatalog, cacheThreadModel, claimCodexLifecycleNotification, captureStructuredUtilityNotification, codexLifecycleThreadId, scheduleStartedThreadCatalogConfirmation, updateCodexCatalogActivity, observeCodexTurnLatency, codexNotificationModel, reportCodexLifecycleNotification, sessionResources, notifyDesktop, t, threadTitle, threadRouter, transcriptUpdateKind, replaceRenderedTurn, renderTranscript, renderComposerState, renderWorkspace, sessionMap, openCodeEventThreadId, bufferOpenCodeHistoryEvent, openCodeLoopGuard, abortRepeatedOpenCodeTerminalLoop, hiddenUtilityThread, markUtilityThreadHidden, scheduleOpenCodeListRefresh, sessionRefKey, forgetStartedThread, deletePersistedSessionState, discardComposerSessionState, invalidateThreadModel, persistPreferences, openCodeCompletionSignal, scheduleOpenCodeStatusReconciliation, updateOpenCodeCatalogActivity, queueStreamingItemPatch, beginOpenCodeHistoryEventBuffer, openCodeHistoryIncludesDeletion, hydrateOpenCodeModelMetadata, markTranscriptHistoryReady, transcriptCaptureSuppressedKeys, endOpenCodeHistoryEventBuffer,
  getBackendIds: () => BACKEND_IDS,
  getOpenCodeHistoryEpoch: () => openCodeHistoryEpoch,
})

const activeCodexConnection = createActiveCodexConnection(state, {
  $, backendDescriptor, codexBackendsNeedingRestartRecovery, sessionDispatch, clearStartedThreadsForBackend, setBackendState, setNativeError, loadBackendModels, loadThreads, sessionManagement, threadCatalogKey, freshThreadModel, selectedThread, refreshSelectedThread, backendSelectionLoads, handleThreadCatalogFailure, reportSessionLifecycle, isArchivedPreview, toast, t, reportClientError, sessionMap, pendingRpcRequests, handleCodexLifecycleNotification, captureStructuredUtilityNotification, sessionRefKey, mergeThreadMetadata, renderWorkspace, renderThreadList, renderTranscript, renderComposerState, hiddenUtilityThread, forgetStartedThread, persistSessionPin, showError, deletePersistedSessionState, threadRouter, discardComposerSessionState, invalidateThreadModel, persistPreferences, composerTrigger, searchComposerSkills, observeCodexTurnLatency, codexNotificationModel, sendRaw, notifyDesktop, markCachedModelValidated, reportCodexLifecycleNotification, sessionResources, transcriptUpdateKind, queueStreamingItemPatch, replaceCompletedItem, replaceRenderedTurn, rpc, cacheThreadModel, threadTitle
})

const settingsApplication = createSettingsApplication(state, {
  $, gatewayFetch, normalizeRightRailWidthRatio, normalizeTypography, typographyDefaults, migrateDefaultFontFamilies, isSupportedBackend, emptyBackendSelections, normalizeAnnotationDrafts, resolveLanguage, normalizeLocalizedTemplates, defaultAnnotationPrompt, persistOpeningMessageState, applySidebarState, getLocale, setLanguage, syncEmbeddedBrowserTranslations, t, applyAppearance, persistPreferences, sessionResources, renderLocalizedUI, toast, annotationPromptDefaults, populateSettingsForm,
  markPreferencesReady: () => { preferencesReady = true },
})

function enableTurnSupervision(ref, turnId, acceptedPrompt = '') {
  if (!isCodexBackend(ref.backend)) { toast(t('Supervision requires a Codex session')); return }
  if (!navigator.locks) { showError(new Error(t('This browser does not support safe supervision ownership'))); return }
  navigator.locks.request(`studio-supervision:${ref.backend}:${ref.id}`, { ifAvailable: true }, async lock => {
    if (!lock) throw new Error(t('This session is already supervised in another window'))
    const latest = messageQueueModel(ref)?.turns.at(-1)
    if (latest?.id !== turnId || latest.status !== 'inProgress') throw new Error(t('No running turn to supervise'))
    turnSupervision.start(ref, turnId, acceptedPrompt)
    do {
      await turnSupervision.tick()
      if (turnSupervision.activeFor(ref)) await new Promise(resolve => setTimeout(resolve, 1000))
    } while (turnSupervision.activeFor(ref))
  }).catch(showError)
}

const turnSupervision = createTurnSupervision({
  snapshot: ref => {
    const model = messageQueueModel(ref)
    return model ? { turns: model.turns, activeTurnId: model.activeTurnId || '',
      blocked: ['disconnected', 'error'].includes(model.status) || Boolean(model.approvals.length || model.interactions.length)
        || state.runningMessageQueues.has(sessionRefKey(ref.backend, ref.id)) } : null
  },
  evaluate: async (job, input, ensureCurrent) => {
    const { ref } = job
    const options = sessionTurnOptions(ref)
    return runHiddenUtilitySession(state, {
      backend: ref.backend, codex: true, cwd: threadForRef(ref)?.cwd || '',
      model: options.model || threadForRef(ref)?.model || '', effort: options.effort || '',
      name: `Studio supervisor ${randomId()}`, instructions: SUPERVISION_INSTRUCTIONS,
      input, outputSchema: SUPERVISION_SCHEMA, validateBeforeStart: true, readCompletedThread: true, ensureCurrent,
      parse: supervisionResult, translateError: t,
      missingTaskMessage: 'Unable to create supervisor evaluation', timeoutMessage: 'Supervisor evaluation timed out',
      rpc: (method, params, timeout) => dispatchBackendRpc(ref.backend, method, params, timeout),
      remove: (backend, params, timeout) => dispatchBackendRpc(backend, 'thread/delete', params, timeout),
      cleanupError: error => console.warn('Supervisor cleanup failed', error), sessionKey: sessionRefKey, turnKey: routerRuntimeKey,
    })
  },
  prepare: async ref => {
    await sessionDispatch.prepareTurn(ref)
    // A cached idle model alone is not authority to send after a connection gap.
    const result = await dispatchBackendRpc(ref.backend, 'thread/read', { threadId: ref.id, includeTurns: true })
    const latest = result?.thread?.turns?.at(-1)
    if (!latest || latest.id !== turnSupervision.get(ref)?.current || latest.status !== 'completed'
      || latest.error || result?.thread?.status?.type === 'active') {
      throw new Error('The native conversation changed; supervision stopped without sending')
    }
  },
  send: async (ref, prompt) => {
    const result = await dispatchBackendRpc(ref.backend, 'turn/start', turnStartParams('codex', threadForRef(ref), {
      threadId: ref.id, clientUserMessageId: randomId(), input: [{ type: 'text', text: prompt }], ...queuedTurnOptions(ref),
    }), 30_000, true)
    const model = messageQueueModel(ref)
    if (result?.turn && model) submissionController.applyTurnAcknowledgement(model, result.turn, ref.id)
    if (result?.turn?.id) {
      await threadRouter.followSupervisedTurn(ref, turnSupervision.get(ref)?.current, String(result.turn.id), model).catch(showError)
    }
    return String(result?.turn?.id || '')
  },
  changed: job => {
    renderComposerTools()
    if (['stopped', 'complete'].includes(job.status)) {
      if (job.status === 'stopped') pauseMessageQueue(job.ref, job.reason)
      toast(`${t(job.status === 'complete' ? 'Supervision complete' : 'Supervision stopped')}: ${job.reason}`)
      renderComposerState()
      if (job.status === 'complete') runNextQueuedMessage(job.ref).catch(showError)
    }
  },
})
window.addEventListener('pagehide', () => turnSupervision.stopAll())

const supervisionWand = '<svg viewBox="0 0 18 18" aria-hidden="true"><path d="m3 15 7.5-7.5 2 2L5 17zM12 2l.7 2.3L15 5l-2.3.7L12 8l-.7-2.3L9 5l2.3-.7zM5 2v3M3.5 3.5h3"/></svg>'
function renderSupervisionMenu(turnId, ref = { backend: state.backend, id: state.selectedId }) {
  if (!isCodexBackend(ref.backend) || !ref.id || isArchivedPreview()) return ''
  const job = turnSupervision.get(ref)
  const enabled = turnSupervision.activeFor(ref) && (job.root === turnId || job.current === turnId)
  const turn = messageQueueModel(ref)?.turns.at(-1)
  if (!enabled && (turn?.id !== turnId || turn.status !== 'inProgress')) return ''
  const label = t(enabled ? 'Stop supervision' : 'Supervise this turn')
  return `<button class="message-copy-button supervision-action${enabled ? ' active' : ''}" type="button" data-supervision-action="${enabled ? 'stop' : 'start'}" data-backend="${escapeHtml(ref.backend)}" data-thread="${escapeHtml(ref.id)}" data-turn="${escapeHtml(turnId)}" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}" aria-pressed="${Boolean(enabled)}">${supervisionWand}</button>`
}
function renderComposerTools() {
  const host = $('#composer-tools-content')
  if (!host) return
  const job = turnSupervision.get({ backend: state.backend, id: state.selectedId })
  const turnId = turnSupervision.activeFor({ backend: state.backend, id: state.selectedId }) ? job.current : state.model.turns.at(-1)?.id
  const action = !isRouterThread() && turnId ? renderSupervisionMenu(turnId) : ''
  host.innerHTML = action ? action.replace('</button>', `<span>${escapeHtml(t(job && turnSupervision.activeFor(job.ref) ? 'Stop supervision' : 'Supervise this turn'))}</span></button>`) : `<small>${escapeHtml(t(isRouterThread() ? 'Use the wand on a running target response' : 'No running turn to supervise'))}</small>`
  for (const button of document.querySelectorAll('.router-target-response [data-supervision-action]')) {
    const ref = { backend: button.dataset.backend, id: button.dataset.thread }
    button.outerHTML = renderSupervisionMenu(button.dataset.turn, ref)
  }
}

const composerActions = createComposerActions(state, {
  $, selectedStateKey, composerDrafts, setCurrentComposerValue, hideComposerMenu, latestAgentResponseText, showError, t, currentBackend, renderComposerState, toast, setComposerDraftValue, gatewayFetch, truncateCharacters, selectedThread, currentTurnOptions, isCodexBackend, randomId, rpc, dispatchBackendRpc, sessionRefKey, routerRuntimeKey, persistMessageQueue, runNextQueuedMessage, sessionRefFromKey, pauseMessageQueue,
})

document.addEventListener('DOMContentLoaded', () => {
  applyRuntimeCopy()
  init().catch(showError)
})
window.addEventListener('error', (event) => reportClientError(event.error || event.message))
window.addEventListener('unhandledrejection', (event) => reportClientError(event.reason))

function reportClientError(error) {
  const summary = error?.message || String(error || 'Unknown WebView error')
  const message = error?.stack && !String(error.stack).includes(summary) ? `${summary}\n${error.stack}` : error?.stack || summary
  gatewayFetch('/studio/client-log', { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: message.slice(0, 16 * 1024) }).catch(() => {})
}

function gatewayToken() {
  const token = window.__CODEX_THREAD_STUDIO_GATEWAY__?.token
  if (!token) throw new Error('Studio gateway credential is unavailable')
  return token
}

function gatewayFetch(input, init = {}) {
  const headers = new Headers(init.headers || {})
  headers.set('Authorization', `Bearer ${gatewayToken()}`)
  return window.fetch(input, { ...init, headers })
}

function gatewayWebSocket(url) {
  return new WebSocket(url, `codex-thread-studio.auth.${gatewayToken()}`)
}

function gatewayEventSource(url) {
  const stream = {
    onopen: null,
    onmessage: null,
    onerror: null,
    closed: false,
    controller: null,
    reconnectTimer: null,
    close() {
      this.closed = true
      this.controller?.abort()
      clearTimeout(this.reconnectTimer)
    },
  }
  const connect = async () => {
    if (stream.closed) return
    stream.controller = new AbortController()
    try {
      const response = await gatewayFetch(url, {
        headers: { Accept: 'text/event-stream' },
        cache: 'no-store',
        signal: stream.controller.signal,
      })
      if (!response.ok || !response.body) throw new Error(`SSE HTTP ${response.status}`)
      stream.onopen?.()
      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ''
      while (!stream.closed) {
        const { value, done } = await reader.read()
        buffer += decoder.decode(value || new Uint8Array(), { stream: !done }).replace(/\r\n?/gu, '\n')
        let boundary
        while ((boundary = buffer.indexOf('\n\n')) >= 0) {
          const record = buffer.slice(0, boundary)
          buffer = buffer.slice(boundary + 2)
          const data = record
            .split('\n')
            .filter((line) => line.startsWith('data:'))
            .map((line) => line.slice(5).replace(/^ /u, ''))
            .join('\n')
          if (data) stream.onmessage?.({ data })
        }
        if (done) throw new Error('SSE stream closed')
      }
    } catch (error) {
      if (stream.closed || error.name === 'AbortError') return
      stream.onerror?.(error)
      stream.reconnectTimer = setTimeout(connect, 1500)
    }
  }
  queueMicrotask(connect)
  return stream
}

async function init() {
  if (!window.__CODEX_THREAD_STUDIO_GATEWAY__) {
    showRemoteAuthenticationRequired()
    return
  }
  if (await loadBackendRegistry() === false) return
  bindUI()
  await loadPreferences()
  // Establish the low-volume cross-backend event barrier while the remaining
  // companion state loads. Active RPC connections are opened separately below
  // and continue to carry full transcript traffic.
  const codexLifecycleConnection = connectCodexLifecycleStream()
  startOpenCodeEventStream()
  setLanguage(state.language)
  startTranslationObserver()
  applyAppearance()
  startMermaidRendering()
  applyBackendCopy()
  await loadBrowserInfo().catch((error) => console.warn('Unable to load Browser info', error))
  syncEmbeddedBrowserTranslations()
  await reviewNotes.loadFavorites().catch(showError)
  // Catalog discovery is independent of the active backend connection. A
  // transient failure in one backend must not leave the whole sidebar empty
  // while the other backend is healthy.
  refreshInactiveCatalog()
  // Codex metadata is not required before its WebSocket connects. OpenCode
  // loads its metadata as part of connectOpenCode(), so it is not requested
  // twice and cannot block initial catalog discovery.
  if (isCodexBackend(state.backend)) loadBackendInfo()
  await codexLifecycleConnection
  connectBackend()
}

async function loadBackendRegistry() {
  try {
    const response = await gatewayFetch('/studio/backends', { cache: 'no-store' })
    if (response.status === 401 && window.__CODEX_THREAD_STUDIO_GATEWAY__?.remote) {
      showRemoteAuthenticationRequired()
      return false
    }
    if (!response.ok) throw new Error(`Backend registry HTTP ${response.status}`)
    const registry = await response.json()
    installBackendRegistry(registry?.backends)
    state.backendRegistry = {
      configPath: String(registry?.configPath || ''),
      configurationError: registry?.configurationError ? String(registry.configurationError) : null,
    }
  } catch (error) {
    installBackendRegistry()
    state.backendRegistry = { configPath: '', configurationError: error.message }
    console.warn('Unable to load backend registry', error)
  }

  const selections = emptyBackendSelections()
  const catalogs = emptyBackendCatalogs()
  state.selectedByBackend = { ...selections, ...state.selectedByBackend }
  state.threadsByBackend = { ...catalogs, ...state.threadsByBackend }
  state.backendInfos = Object.fromEntries(BACKEND_IDS.map((backend) => [backend, state.backendInfos[backend] || null]))
  state.backendModels = Object.fromEntries(BACKEND_IDS.map((backend) => [backend, state.backendModels[backend] || []]))
  state.appServerGenerations = Object.fromEntries(BACKEND_IDS.map((backend) => [backend, state.appServerGenerations[backend] ?? null]))
  state.backendStates = Object.fromEntries(BACKEND_IDS.map((backend) => [backend, state.backendStates[backend] || {
    kind: 'idle', label: backendDescriptor(backend).name, caption: 'Connect on demand',
  }]))
  for (const backend of BACKEND_IDS) {
    if (isCodexBackend(backend) && !sessionDispatch.supports(backend)) {
      sessionDispatch.register(backend, codexDispatchAdapter())
    }
  }
  renderBackendChoices()
  return true
}

function showRemoteAuthenticationRequired() {
  state.ready = false
  const message = t('SSH access has expired or is missing. Open the latest URL printed by the SSH service, including #token=…; refreshing the old page cannot renew access.')
  $('#empty-workspace').classList.add('hidden')
  $('#native-workspace').classList.remove('hidden')
  $('#native-error').classList.remove('hidden')
  $('#native-error-title').textContent = t('SSH authentication required')
  $('#native-error-message').textContent = message
  $('#native-error').setAttribute('role', 'alert')
  $('#retry-native').classList.add('hidden')
  $('#native-connection').textContent = t('SSH authentication required')
  $('#send-message').disabled = true
  $('#continue-thread').disabled = true
}

function renderBackendChoices() {
  const select = $('#new-thread-backend')
  if (!select) return
  const selected = select.value
  select.innerHTML = backendDescriptors()
    .map((descriptor) => `<option value="${escapeHtml(descriptor.id)}">${escapeHtml(descriptor.name)}</option>`)
    .join('')
  if (isSupportedBackend(selected)) select.value = selected
}

function bindUI() {
  window.addEventListener('focus', refreshActiveCodexCatalogOnFocus)
  workspaceTools.bind()
  sessionResources.bind()
  sessionManagement.bind()
  threadRouter.bind()
  sessionMap.bind()
  reviewNotes.bind()
  documentWorkspace.bind()
  $('#new-thread').addEventListener('click', openNewThreadDialog)
  $('#studio-menu-button').addEventListener('click', () => {
    toggleActionMenu('studio-menu', 'studio-menu-button')
    if (!$('#studio-menu').classList.contains('hidden')) {
      reviewNotes.refreshGlobalFavoriteCount().catch(error => console.warn('Unable to refresh global favorite count', error))
    }
  })
  $('#toggle-sidebar').addEventListener('click', toggleSidebar)
  $('#empty-new-thread').addEventListener('click', openNewThreadDialog)
  $('#close-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#cancel-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#new-thread-form').addEventListener('submit', createThread)
  $('#new-thread-backend').addEventListener('change', handleNewThreadBackendChange)
  $$('.thread-filter').forEach((button) => button.addEventListener('click', () => {
    state.filter = button.dataset.filter
    renderThreadList()
  }))
  $('#thread-more-button').addEventListener('click', () => toggleActionMenu('thread-more-menu', 'thread-more-button'))
  $('#refresh-thread').addEventListener('click', () => {
    closeActionMenus()
    refreshSelectedThread()
  })
  $('#thread-info').addEventListener('click', () => {
    closeActionMenus()
    openThreadInfo()
  })
  $('#project-environment-action').addEventListener('click', () => openEnvironmentDialog())
  $('#environment-form').addEventListener('submit', saveProjectEnvironment)
  $('#close-environment-dialog').addEventListener('click', closeEnvironmentDialog)
  $('#cancel-environment').addEventListener('click', closeEnvironmentDialog)
  $('#environment-secret-names').addEventListener('click', toggleEnvironmentSecretRemoval)
  $('#environment-variables').addEventListener('input', updateEnvironmentDraftSummary)
  $('#environment-secrets').addEventListener('input', updateEnvironmentDraftSummary)
  $('#environment-cache-variables').addEventListener('input', updateEnvironmentDraftSummary)
  $$('input[name="environment-network-policy"]').forEach((input) => input.addEventListener('change', updateEnvironmentDraftSummary))
  $('#rename-thread').addEventListener('click', () => {
    closeActionMenus()
    openRenameThreadDialog()
  })
  $('#rename-thread-form').addEventListener('submit', renameSelectedThread)
  $('#close-rename-thread').addEventListener('click', closeRenameThreadDialog)
  $('#cancel-rename-thread').addEventListener('click', closeRenameThreadDialog)
  $('#fork-thread').addEventListener('click', () => {
    closeActionMenus()
    forkSelectedThread()
  })
  $('#pin-thread').addEventListener('click', () => {
    closeActionMenus()
    toggleSelectedThreadPin()
  })
  $('#archive-thread').addEventListener('click', () => {
    closeActionMenus()
    archiveSelectedThread()
  })
  $('#delete-thread').addEventListener('click', () => {
    closeActionMenus()
    deleteSelectedThread()
  })
  $('#retry-native').addEventListener('click', connectBackend)
  $('#composer-form').addEventListener('submit', sendComposer)
  $('#composer-input').addEventListener('input', handleComposerInput)
  $('#composer-input').addEventListener('keydown', handleComposerKeydown)
  $('#composer-add-image').addEventListener('click', () => $('#composer-image-input').click())
  $('#composer-tools').addEventListener('toggle', renderComposerTools)
  $('#composer-tools-content').addEventListener('click', handleTranscriptClick)
  $('#composer-image-input').addEventListener('change', (event) => {
    addComposerImages(event.target.files).catch(showError)
    event.target.value = ''
  })
  $('#composer-input').addEventListener('paste', handleComposerImagePaste)
  $('#composer-images').addEventListener('click', removeComposerImage)
  $('#composer-form').addEventListener('dragenter', handleComposerImageDrag)
  $('#composer-form').addEventListener('dragover', handleComposerImageDrag)
  $('#composer-form').addEventListener('dragleave', handleComposerImageDragLeave)
  $('#composer-form').addEventListener('drop', handleComposerImageDrop)
  $('#composer-menu').addEventListener('mousedown', (event) => event.preventDefault())
  $('#composer-menu').addEventListener('click', handleComposerMenuClick)
  $('#interrupt-turn').addEventListener('click', interruptTurn)
  $('#queue-message').addEventListener('click', queueComposerMessage)
  $('#continue-thread').addEventListener('click', handleContinueAction)
  $('#resume-message-queue').addEventListener('click', resumeSelectedMessageQueue)
  $('#composer-queue-items').addEventListener('click', handleMessageQueueClick)
  $('#edit-queued-message-form').addEventListener('submit', saveEditedQueuedMessage)
  $('#close-edit-queued-message').addEventListener('click', closeQueuedMessageEditor)
  $('#cancel-edit-queued-message').addEventListener('click', closeQueuedMessageEditor)
  $('#turn-navigator-list').addEventListener('click', handleTurnNavigatorClick)
  $('#open-browser-workspace').addEventListener('click', () => openGlobalBrowser().catch(showError))
  $('#transcript').addEventListener('scroll', handleTranscriptScroll, { passive: true })
  $('#transcript').addEventListener('wheel', handleTranscriptUserTakeover, { passive: true })
  $('#transcript').addEventListener('touchstart', handleTranscriptUserTakeover, { passive: true })
  $('#transcript').addEventListener('pointerdown', handleTranscriptUserTakeover, { passive: true })
  $('#transcript').addEventListener('keydown', handleTranscriptKeyboardTakeover)
  $('#transcript').addEventListener('touchend', finishTranscriptUserTakeover, { passive: true })
  $('#transcript').addEventListener('touchcancel', finishTranscriptUserTakeover, { passive: true })
  window.addEventListener('pointerup', finishTranscriptUserTakeover, { passive: true })
  window.addEventListener('pointercancel', finishTranscriptUserTakeover, { passive: true })
  $('#artifact-content').addEventListener('click', handleTranscriptClick)
  for (const resizer of $$('.app-right-rail-resizer')) {
    resizer.addEventListener('pointerdown', beginRightRailResize)
    resizer.addEventListener('pointermove', continueRightRailResize)
    resizer.addEventListener('pointerup', finishRightRailResize)
    resizer.addEventListener('pointercancel', finishRightRailResize)
    resizer.addEventListener('dblclick', resetRightRailWidth)
    resizer.addEventListener('keydown', handleRightRailResizeKey)
  }
  $('#transcript').addEventListener('click', handleTranscriptClick)
  document.addEventListener('click', (event) => handleMarkdownActionClick(event).catch(reportClientError))
  window.addEventListener('resize', () => {
    scheduleTurnNavigatorSync()
    applyRightRailWidth()
    workspaceTools.resize()
  })
  $('#connections-button').addEventListener('click', openConnectionsDialog)
  $('#close-connections').addEventListener('click', () => $('#connections-dialog').close())
  $('#settings-button').addEventListener('click', () => {
    closeActionMenus()
    openSettings()
  })
  $('#about-button').addEventListener('click', openBackendDialog)
  $('#close-settings').addEventListener('click', () => $('#settings-dialog').close())
  $('#cancel-settings').addEventListener('click', () => $('#settings-dialog').close())
  $('#settings-navigation').addEventListener('click', handleSettingsNavigationClick)
  $('#settings-navigation').addEventListener('keydown', handleSettingsNavigationKeydown)
  $('#translation-engine').addEventListener('change', handleTranslationEngineChange)
  $('#settings-form').addEventListener('submit', saveSettings)
  $('#reset-settings').addEventListener('click', resetSettings)
  $('#close-backend').addEventListener('click', () => $('#backend-dialog').close())
  $('#close-command').addEventListener('click', () => $('#command-dialog').close())
  $('#close-activity-log').addEventListener('click', () => $('#activity-log-dialog').close())
  $('#done-activity-log').addEventListener('click', () => $('#activity-log-dialog').close())
  $('#close-thread-info').addEventListener('click', () => $('#thread-info-dialog').close())
  $('#done-thread-info').addEventListener('click', saveThreadInfo)
  $('#copy-opening-message').addEventListener('click', () => copyOpeningMessage().catch(showError))

  document.addEventListener('mousedown', (event) => {
    if (state.composerMenu.trigger?.confirmedPicker && !event.target.closest('#router-choose-target, #composer-menu')) hideComposerMenu()
    if (!event.target.closest('#selection-popover, .content-menu-anchor')) reviewNotes.hideSelection()
    if (!event.target.closest('.menu-anchor, .composer-tools, .composer-attachment-control, #composer-supervision-status')) closeActionMenus()
    if (!event.target.closest('#session-map-item-menu, .session-map-row-menu')) sessionMap.closeItemMenu()
    if (!event.target.closest('#thread-content-search')) sessionManagement.search.hideResults()
  })
  document.addEventListener('keydown', (event) => {
    const modifier = event.ctrlKey || event.metaKey
    if (modifier && event.key.toLowerCase() === 'f' && selectedThread() && !event.target.closest('.artifact-rail, .workspace-files-rail, .workspace-terminal-rail, .workspace-review-rail, .embedded-browser-rail')) {
      event.preventDefault()
      sessionManagement.search.open()
    } else if (modifier && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      openNewThreadDialog()
    } else if (modifier && event.key.toLowerCase() === 'b') {
      event.preventDefault()
      toggleSidebar()
    } else if (event.key === '/' && !isTypingTarget(event.target)) {
      event.preventDefault()
      $('#thread-search').focus()
    } else if (event.key === 'Escape' && state.threadSearch.open) {
      sessionManagement.search.close()
    } else if (event.key === 'Escape') {
      const artifactWasOpen = !$('#artifact-rail').classList.contains('hidden')
      reviewNotes.hideSelection()
      closeActionMenus()
      reviewNotes.closeAnnotations()
      reviewNotes.closeFavorites()
      sessionResources.close()
      sessionMap.closeItemMenu()
      if (artifactWasOpen) closeArtifactRail({ restoreWorkspace: false })
      workspaceTools.close()
    }
  })
}

function toggleSidebar() {
  state.sidebarCollapsed = !state.sidebarCollapsed
  applySidebarState()
  persistPreferences()
}

function applySidebarState() {
  $('.app-shell').classList.toggle('sidebar-collapsed', state.sidebarCollapsed)
  const button = $('#toggle-sidebar')
  button.setAttribute('aria-expanded', String(!state.sidebarCollapsed))
  button.title = t(state.sidebarCollapsed ? 'Expand sidebar (Ctrl+B)' : 'Collapse sidebar (Ctrl+B)')
  button.setAttribute('aria-label', t(state.sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'))
  button.querySelector('span').textContent = state.sidebarCollapsed ? '›' : '‹'
  setTimeout(applyRightRailWidth, 220)
}

async function browserRequest(path, { method = 'GET', body } = {}) {
  const response = await gatewayFetch(path, {
    method,
    cache: 'no-store',
    headers: body == null ? {} : { 'Content-Type': 'application/json' },
    body: body == null ? undefined : JSON.stringify(body),
  })
  const text = await response.text()
  let value = null
  try { value = text ? JSON.parse(text) : null } catch { value = text }
  if (!response.ok) throw new Error(value?.error?.message || value?.message || String(value || `HTTP ${response.status}`))
  return value
}

async function loadBrowserInfo() {
  state.browserInfo = await browserRequest('/studio/browser')
  renderBrowserMenuStatus()
  return state.browserInfo
}

function renderBrowserMenuStatus() {
  const element = $('#browser-menu-status')
  if (!element) return
  const available = usesEmbeddedBrowser()
  element.className = `browser-menu-status ${state.embeddedBrowserVisible ? 'online' : available ? 'offline' : 'error'}`
  element.querySelector('small').textContent = 'embedded'
  const label = t(!available ? 'Unavailable' : state.embeddedBrowserVisible ? 'Visible' : state.embeddedBrowserLoaded ? 'Hidden' : 'Not launched')
  const button = $('#open-browser-workspace')
  button.disabled = !available
  button.title = `${t('Browser')} · ${label} · embedded`
  button.setAttribute('aria-label', button.title)
}

async function openGlobalBrowser() {
  closeActionMenus()
  if (!usesEmbeddedBrowser()) throw new Error(t('The embedded browser is unavailable on this platform'))
  const width = currentRightRailPixelWidth()
  activateRightWorkspace('browser')
  dispatchEmbeddedBrowserAction(`studio-action://show-browser?width=${width}`)
}

async function openBrowserUrl(url) {
  if (!usesEmbeddedBrowser()) {
    if (state.remoteClient) {
      // With noopener, a successful open also returns null. Do not mistake that
      // security behavior for popup blocking.
      window.open(String(url || ''), '_blank', 'noopener,noreferrer')
      return
    }
    throw new Error(t('The embedded browser is unavailable on this platform'))
  }
  const width = currentRightRailPixelWidth()
  activateRightWorkspace('browser')
  dispatchEmbeddedBrowserAction(`studio-action://open-browser?url=${encodeURIComponent(String(url || ''))}&width=${width}`)
}

async function openSessionResource(resource) {
  if (!resource || resource.state === 'blocked') {
    throw new Error(resource?.reason || t('This resource was blocked by the security policy'))
  }
  if (resource.target?.url) {
    await openBrowserUrl(resource.target.url)
    return
  }
  const target = resource.target || {}
  if (!target.path || !target.workspaceRoot) throw new Error(t('The resource has no file target that can be opened'))
  if (resource.kind === 'directory') {
    await workspaceTools.reveal(target.path)
    return
  }
  if (!reviewableFileKind({ path: target.path })) {
    await workspaceTools.reveal(target.path)
    toast(t('This file type cannot be previewed; it has been revealed in Files.'))
    return
  }
  await openArtifact({ root: target.workspaceRoot, path: target.path }, { returnTool: 'resources' })
  if (target.line) jumpArtifactToLine(target.line, target.column)
}

async function translateSelectionWithCurrentBackend(value) {
  const text = String(value || '').trim().slice(0, 16_000)
  if (!text) throw new Error(t('Select text to translate first'))
  if (!state.selectedId) throw new Error(t('The current backend is not ready for translation'))

  const profile = currentSelectionTranslationProfile()
  const { backend, model, effort } = profile
  const cacheKey = translationCacheKey({ backend, model, effort, text })
  const cached = state.selectionTranslationCache.get(cacheKey)
  if (cached) {
    state.selectionTranslationCache.delete(cacheKey)
    state.selectionTranslationCache.set(cacheKey, cached)
    return cached
  }
  if (profile.engine === 'ollama') {
    return translateSelectionWithOllama(text, profile, cacheKey)
  }
  if (!state.ready) throw new Error(t('The current backend is not ready for translation'))

  const generation = state.socketGeneration
  const cwd = selectedThread()?.cwd || ''

  const translation = await runHiddenUtilitySession(state, {
    backend, codex: isCodexBackend(backend), cwd, model, effort,
    name: `Studio translation ${randomId()}`,
    instructions: SELECTION_TRANSLATION_INSTRUCTIONS,
    input: selectionTranslationInput(text),
    outputSchema: SELECTION_TRANSLATION_SCHEMA,
    validateBeforeStart: false,
    ensureCurrent: () => ensureTranslationBackend(backend, generation),
    parse: translationTurnState,
    translateError: t,
    missingTaskMessage: t('The current backend did not create a translation task'),
    timeoutMessage: t('Translation timed out'),
    rpc,
    remove: (targetBackend, params, timeoutMs) => dispatchBackendRpc(targetBackend, 'thread/delete', params, timeoutMs),
    cleanupError: error => console.warn('Unable to remove the hidden translation session', error),
    sessionKey: sessionRefKey,
    turnKey: routerRuntimeKey,
  })
  const result = {
    translation: translation.translation,
    sourcePronunciation: translation.sourcePronunciation,
    translationPronunciation: translation.translationPronunciation,
  }
  state.selectionTranslationCache.set(cacheKey, result)
  while (state.selectionTranslationCache.size > 64) {
    state.selectionTranslationCache.delete(state.selectionTranslationCache.keys().next().value)
  }
  return result
}

async function translateSelectionWithOllama(text, profile, cacheKey) {
  const response = await gatewayFetch('/studio/ollama/translate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: profile.model, text }),
  })
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) {
    throw new Error(payload?.error?.message || t('Local Ollama translation failed'))
  }
  const result = {
    translation: String(payload.translation || '').trim(),
    sourcePronunciation: String(payload.sourcePronunciation || '').trim(),
    translationPronunciation: String(payload.translationPronunciation || '').trim(),
    model: String(payload.model || profile.model),
    totalDurationMs: Number(payload.totalDurationMs || 0),
    loadDurationMs: Number(payload.loadDurationMs || 0),
  }
  if (!result.translation) throw new Error(t('The translation backend returned an empty translation'))
  state.selectionTranslationCache.set(cacheKey, result)
  while (state.selectionTranslationCache.size > 64) {
    state.selectionTranslationCache.delete(state.selectionTranslationCache.keys().next().value)
  }
  return result
}

function currentSelectionTranslationProfile() {
  if (state.translation.engine === 'ollama') {
    const model = String(state.translation.ollamaModel || 'gemma3:4b')
    return {
      engine: 'ollama',
      backend: 'ollama',
      backendName: t('Local Ollama'),
      model,
      displayModel: model,
      modelSource: 'local',
      effort: '',
    }
  }
  const backend = state.backend
  const descriptor = backendDescriptor(backend)
  const translationModel = String(state.translation.models[backend] || '')
  const sessionModel = String(currentTurnOptions().model || '')
  const model = translationModel || sessionModel
  const defaultModel = (state.backendModels[backend] || [])
    .find((entry) => entry?.isDefault)
  const displayModel = model
    || String(defaultModel?.model || defaultModel?.id || '')
    || t('{backend} default', { backend: descriptor.name })
  return {
    engine: 'backend',
    backend,
    backendName: descriptor.name,
    model,
    displayModel,
    modelSource: translationModel ? 'translation' : sessionModel ? 'session' : 'backend',
    effort: state.translation.efforts[backend] ?? (isCodexBackend(backend) ? 'low' : ''),
  }
}

function ensureTranslationBackend(backend, generation) {
  if (state.backend !== backend || state.socketGeneration !== generation || !state.ready) {
    throw new Error(t('Translation stopped because the current backend changed'))
  }
}

function markUtilityThreadHidden(backend, threadId) {
  const key = sessionRefKey(backend, threadId)
  state.hiddenUtilityThreads.add(key)
  if (isCodexBackend(backend)) state.hiddenCodexThreads.add(key)
}

function hiddenUtilityThread(backend, thread) {
  const id = String(thread?.id || '')
  const name = String(thread?.name || thread?.title || '')
  return Boolean(
    (id && state.hiddenUtilityThreads.has(sessionRefKey(backend, id)))
    || (name && state.hiddenUtilityThreadNames.has(`${backend}:${name}`)),
  )
}

function captureStructuredUtilityNotification(backend, message) {
  const params = message?.params || {}
  const threadId = String(params.threadId || params.thread?.id || params.turn?.threadId || '')
  const turnId = String(params.turnId || params.turn?.id || '')
  let task = threadId
    ? state.structuredUtilityTasks.get(sessionRefKey(backend, threadId))
    : null
  if (!task && turnId) {
    task = [...state.structuredUtilityTasks.values()]
      .find((candidate) => candidate.backend === backend && candidate.turnId === turnId)
  }
  if (!task) return false
  if (turnId && task.turnId && task.turnId !== turnId) return false
  if (turnId) task.turnId ||= turnId
  if (message.id != null && message.method) {
    sendRaw({ id: message.id, error: { code: -32601, message: 'Studio utility tasks do not support interactive requests' } })
    return true
  }
  applyCodexNotification(task.model, message)
  return true
}

function openSessionResourceSource(occurrence) {
  if (!occurrence?.turnId) return
  beginTranscriptProgrammaticNavigation()
  transcriptPresentationCache.showTurn(presentationThreadKey(), state.model, occurrence.turnId)
  renderTranscript()
  requestAnimationFrame(() => {
    const turn = [...$('#transcript').querySelectorAll('.turn[data-turn-id]')]
      .find((candidate) => candidate.dataset.turnId === String(occurrence.turnId))
    const item = turn && [...turn.querySelectorAll('[data-item-id]')]
      .find((candidate) => candidate.dataset.itemId === String(occurrence.itemId))
    const target = item || turn
    if (!target) return
    target.scrollIntoView({ block: 'center', behavior: 'smooth' })
    target.classList.add('resource-source-highlight')
    setTimeout(() => target.classList.remove('resource-source-highlight'), 1800)
  })
}

function currentRightRailPixelWidth() {
  const bounds = appRightRailWidthBounds()
  return Math.round(Math.max(480, Math.min(bounds.max, bounds.available * state.rightRailWidthRatio)))
}

function dispatchEmbeddedBrowserAction(url) {
  if (window.__studioEmbeddedBrowserIpc && window.ipc?.postMessage) {
    window.ipc.postMessage(JSON.stringify({ type: 'studio-browser-action', url }))
    return
  }
  window.location.href = url
}

const embeddedBrowserTranslationSources = [
  'Browser tabs', 'New tab', 'New tab (Ctrl+T)', 'Hide browser', 'Back', 'Forward', 'Reload',
  'Address', 'Zoom', 'Zoom out', 'Reset to 100%', 'Zoom in', 'Fit page width', 'Comment',
  'Comment on selection', 'More browser actions', 'Close tab (Ctrl+W)', 'Copy current link',
  'Browser information', 'Downloads', 'Open downloads folder', 'Exit browser', 'No downloads yet',
  'tabs', 'Profile', 'Cookies', 'Local Storage', 'WebKit Cache', 'Storage & cleanup',
  'WebView2 Profile', 'Cache', 'Engine', 'TLS',
  'Cache, cookies, Local Storage, and other site data are stored on disk. Tab navigation history stays in memory and disappears when Studio exits.',
  'Clear browsing data…', 'Open folder', 'Downloading', 'Completed', 'Failed', 'Copy', 'Copied',
  'Clear all embedded browser data?',
  'This clears shared site data including cache, cookies, Local Storage, IndexedDB, and service workers. Website sign-ins may be lost. This action cannot be undone.',
  'Cancel', 'Clear',
  'Cleanup requested. The current page may need to be reloaded, and website sign-ins may be lost.',
]

function syncEmbeddedBrowserTranslations() {
  if (!usesEmbeddedBrowser()) return
  const messages = Object.fromEntries(embeddedBrowserTranslationSources.map((source) => [source, t(source)]))
  dispatchEmbeddedBrowserAction(`studio-action://set-browser-translations?messages=${encodeURIComponent(JSON.stringify(messages))}`)
  syncEmbeddedBrowserTypography()
}

let syncedEmbeddedBrowserTypography = ''
function syncEmbeddedBrowserTypography() {
  if (!usesEmbeddedBrowser()) return
  const profile = {
    fontFamily: state.typography.uiFontFamily,
    fontSize: state.typography.uiFontSize,
    fontWeight: state.typography.uiFontWeight,
  }
  const serialized = JSON.stringify(profile)
  if (serialized === syncedEmbeddedBrowserTypography) return
  dispatchEmbeddedBrowserAction(`studio-action://set-browser-typography?profile=${encodeURIComponent(serialized)}`)
  syncedEmbeddedBrowserTypography = serialized
}

function usesEmbeddedBrowser() {
  return state.browserInfo?.presentation === 'embedded-webview'
}

function openEmbeddedBrowserComment(selection) {
  const excerpt = String(selection?.text || '').trim().slice(0, 16000)
  if (!excerpt) return toast(t('Select text on the web page first'), 'error')
  if (!state.selectedId) return toast(t('Select a session first'), 'error')
  reviewNotes.openCommentForSelection({
    quote: excerpt,
    itemId: null,
    turnId: null,
    source: browserCommentSource({
      url: selection.url,
      title: selection.title,
    }),
  })
}

window.__studioEmbeddedBrowser = Object.freeze({
  setVisible(visible) {
    const requestedVisible = Boolean(visible)
    state.embeddedBrowserVisible = requestedVisible && state.activeRightWorkspace === 'browser'
    if (requestedVisible && state.activeRightWorkspace !== 'browser') {
      window.location.href = 'studio-action://hide-browser'
    }
    renderBrowserMenuStatus()
  },
  setRuntimeLoaded(loaded) {
    state.embeddedBrowserLoaded = Boolean(loaded)
    renderBrowserMenuStatus()
  },
  setWidth(metrics) {
    const value = Math.round(Number(typeof metrics === 'object' ? metrics?.width : metrics))
    const totalWidth = Math.round(Number(typeof metrics === 'object' ? metrics?.totalWidth : 0))
    if (!Number.isFinite(value) || value < 480 || value > 2400) return
    const shellWidth = $('.app-shell')?.clientWidth || window.innerWidth
    const sidebarWidth = state.sidebarCollapsed ? 0 : $('#sidebar')?.getBoundingClientRect().width || 0
    const dividerWidth = $('#sidebar-divider')?.offsetWidth || 0
    const bounds = rightRailWidthBounds({
      containerWidth: totalWidth > 0 ? totalWidth : shellWidth + value,
      sidebarWidth,
      dividerWidth,
      nominalMinWidth: 480,
    })
    const sharedWidth = Math.round(Math.max(bounds.min, Math.min(bounds.max, value)))
    state.rightRailWidthRatio = normalizeRightRailWidthRatio(sharedWidth / bounds.available)
    document.documentElement.style.setProperty('--right-rail-width', `${sharedWidth}px`)
    state.browser.embeddedWidth = sharedWidth
    if (sharedWidth !== value && state.activeRightWorkspace === 'browser') {
      window.location.href = `studio-action://resize-browser?width=${sharedWidth}`
    }
    clearTimeout(embeddedBrowserWidthTimer)
    embeddedBrowserWidthTimer = setTimeout(persistPreferences, 250)
  },
  openComment(selection) {
    openEmbeddedBrowserComment(selection)
  },
  notify(message) {
    toast(t(String(message || '')), 'error')
  },
})

window.__studioDeveloper = Object.freeze({
  openArtifact(file) {
    if ($('#settings-dialog').open) $('#settings-dialog').close()
    openArtifact(file, { allowDetachedRoot: true }).catch(showError)
  },
  openWorkspace(root, tool) {
    workspaceTools.openForDebug(root, tool).catch(showError)
  },
  openResources(root) {
    sessionResources.openForDebug(String(root || ''))
  },
  openEnvironmentSettings(root = '') {
    openEnvironmentDialog(String(root || ''))
  },
  click(selector) {
    document.querySelector(String(selector || ''))?.click()
  },
  drag(selector, deltaX = 0, deltaY = 0) {
    const target = document.querySelector(String(selector || ''))
    if (!target) return
    const rect = target.getBoundingClientRect()
    const startX = rect.left + rect.width / 2
    const startY = rect.top + rect.height / 2
    target.dispatchEvent(new MouseEvent('mousedown', {
      bubbles: true, cancelable: true, view: window, button: 0, buttons: 1, clientX: startX, clientY: startY,
    }))
    window.dispatchEvent(new MouseEvent('mousemove', {
      bubbles: true, cancelable: true, view: window, button: 0, buttons: 1,
      clientX: startX + Number(deltaX || 0), clientY: startY + Number(deltaY || 0),
    }))
    window.dispatchEvent(new MouseEvent('mouseup', {
      bubbles: true, cancelable: true, view: window, button: 0, buttons: 0,
      clientX: startX + Number(deltaX || 0), clientY: startY + Number(deltaY || 0),
    }))
  },
  input(selector, value) {
    const target = document.querySelector(String(selector || ''))
    if (!target || !('value' in target)) return
    target.value = String(value ?? '')
    target.dispatchEvent(new Event('input', { bubbles: true }))
  },
})

function normalizeRightRailWidthRatio(value) {
  return Math.min(0.65, Math.max(0.2, Number(value) || 0.44))
}

function appRightRailWidthBounds() {
  const shell = $('.app-shell')
  const sidebarWidth = state.sidebarCollapsed ? 0 : $('#sidebar').getBoundingClientRect().width
  const nominalSidebarWidth = Math.max(480, Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--sidebar-width')) || 310)
  return rightRailWidthBounds({
    containerWidth: shell.clientWidth,
    sidebarWidth,
    dividerWidth: $('#sidebar-divider').offsetWidth,
    nominalMinWidth: nominalSidebarWidth,
  })
}

function applyRightRailWidth(ratio = state.rightRailWidthRatio, { updateState = false } = {}) {
  const preferredRatio = normalizeRightRailWidthRatio(ratio)
  const { available, min, max } = appRightRailWidthBounds()
  const actual = Math.min(max, Math.max(min, available * preferredRatio))
  document.documentElement.style.setProperty('--right-rail-width', `${actual}px`)
  for (const resizer of $$('.app-right-rail-resizer')) {
    resizer.setAttribute('aria-valuemin', String(min))
    resizer.setAttribute('aria-valuemax', String(Math.round(max)))
    resizer.setAttribute('aria-valuenow', String(Math.round(actual)))
  }
  if (updateState) state.rightRailWidthRatio = normalizeRightRailWidthRatio(actual / available)
  workspaceTools.resize()
  return actual
}

function beginRightRailResize(event) {
  if (event.button !== 0) return
  event.preventDefault()
  window.getSelection?.()?.removeAllRanges()
  rightRailResize = { pointerId: event.pointerId, rail: event.currentTarget.parentElement }
  event.currentTarget.setPointerCapture(event.pointerId)
  document.body.classList.add('resizing-right-rail')
  continueRightRailResize(event)
}

function continueRightRailResize(event) {
  if (!rightRailResize || rightRailResize.pointerId !== event.pointerId) return
  const right = rightRailResize.rail?.getBoundingClientRect().right || window.innerWidth
  const { available } = appRightRailWidthBounds()
  applyRightRailWidth((right - event.clientX) / available, { updateState: true })
}

function finishRightRailResize(event) {
  if (!rightRailResize || rightRailResize.pointerId !== event.pointerId) return
  rightRailResize = null
  document.body.classList.remove('resizing-right-rail')
  if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  persistPreferences()
}

function resetRightRailWidth() {
  state.rightRailWidthRatio = 0.44
  applyRightRailWidth()
  persistPreferences()
}

function handleRightRailResizeKey(event) {
  const step = event.shiftKey ? 64 : 24
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  const { available, min, max } = appRightRailWidthBounds()
  const current = event.currentTarget.parentElement?.getBoundingClientRect().width || min
  const next = event.key === 'Home' ? min
    : event.key === 'End' ? max
      : current + (event.key === 'ArrowLeft' ? step : -step)
  applyRightRailWidth(next / available, { updateState: true })
  persistPreferences()
}

function toggleActionMenu(menuId, buttonId) {
  const menu = $(`#${menuId}`)
  const opening = menu.classList.contains('hidden')
  closeActionMenus()
  menu.classList.toggle('hidden', !opening)
  $(`#${buttonId}`).setAttribute('aria-expanded', String(opening))
}

function closeActionMenus() {
  $$('.action-menu').forEach((menu) => menu.classList.add('hidden'))
  $$('.menu-anchor [aria-expanded]').forEach((button) => button.setAttribute('aria-expanded', 'false'))
  $('#composer-tools').open = false
}

async function loadBackendInfo(backend = state.backend) {
  const descriptor = backendDescriptor(backend)
  try {
    const response = await gatewayFetch(descriptor.infoPath, { cache: 'no-store' })
    const info = await response.json()
    state.backendInfos[backend] = response.ok
      ? info
      : { ...info, error: info?.error || `HTTP ${response.status}` }
    if (info?.hostPlatform) state.hostPlatform = info.hostPlatform
  } catch (error) {
    state.backendInfos[backend] = { binary: descriptor.binary, protocol: descriptor.protocol, transport: descriptor.transport, error: error.message }
  }
  if (backend === state.backend) {
    state.backendInfo = state.backendInfos[backend]
    applyBackendCopy()
  }
  return state.backendInfos[backend]
}

function currentBackend() {
  return backendDescriptor(state.backend)
}

function waitForOpenCodeEventStream(timeoutMs = 1_500) {
  return waitForEventStream(openCodeStreamState, timeoutMs)
}

function renderOpenCodeConnectionState(kind, label, caption) {
  state.backendStates.opencode = { kind, label, caption }
  if (state.backend === 'opencode') {
    const connection = $('#thread-connection')
    if (connection) connection.className = `thread-connection ${kind}`
    const copy = kind === 'online' ? 'Connected' : kind === 'checking' ? 'Connecting…' : 'Disconnected'
    $('#native-connection').textContent = t(copy)
  }
  if ($('#connections-dialog').open) renderConnectionsDialog()
  if ($('#backend-dialog').open) renderBackendDialog()
}

function startOpenCodeEventStream() {
  return connectEventStream(openCodeStreamState, {
    open: () => gatewayEventSource('/opencode/global/event'),
    connected: needsReconciliation => {
      renderOpenCodeConnectionState('online', 'OpenCode Server', 'Background event connection')
      if (needsReconciliation) {
        openCodeHistoryEpoch += 1
        scheduleOpenCodeListRefresh({ forceSelectedHistory: state.backend === 'opencode' })
        scheduleOpenCodeLifecycleReconciliation()
      }
      reportSessionLifecycle('opencode-event-stream', {
        state: needsReconciliation ? 'reconnected' : 'connected',
        historyEpoch: openCodeHistoryEpoch,
      })
    },
    message: handleOpenCodeServerEvent,
    invalid: (error, data) => console.error('Invalid OpenCode SSE event', error, data),
    disconnected: () => renderOpenCodeConnectionState('checking', 'Reconnecting to OpenCode', 'SSE event stream'),
  })
}

function scheduleOpenCodeLifecycleReconciliation() {
  return backgroundSessions.scheduleOpenCodeLifecycleReconciliation()
}

function connectCodexLifecycleStream() {
  return connectLifecycleStream(codexLifecycleConnection, {
    open: () => {
      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
      return gatewayWebSocket(`${protocol}//${location.host}/ws/codex-lifecycle`)
    },
    invalid: (error, data) => console.error('Invalid cross-backend Codex lifecycle message', error, data),
    report: details => reportSessionLifecycle('codex-lifecycle-stream', details),
    reconcile: scheduleCodexLifecycleReconciliation,
    supports: isCodexBackend,
    status: handleInactiveCodexAppServerStatus,
    lag: handleCodexLifecycleLag,
    notification: handleCodexLifecycleNotification,
  })
}

function invalidateCodexBackendModelValidation(backend) {
  return backgroundSessions.invalidateCodexBackendModelValidation(backend)
}

function handleInactiveCodexAppServerStatus(backend, message) {
  return backgroundSessions.handleInactiveCodexAppServerStatus(backend, message)
}

function handleCodexLifecycleLag(backend, message) {
  return backgroundSessions.handleCodexLifecycleLag(backend, message)
}

function scheduleCodexLifecycleReconciliation() {
  return backgroundSessions.scheduleCodexLifecycleReconciliation()
}

async function reconcileCodexLifecycleBackend(backend) {
  return backgroundSessions.reconcileCodexLifecycleBackend(backend)
}

function refreshOffscreenCodexHistoryAfterCompletion(backend, threadId, sourceModel, { advanceQueue = false } = {}) {
  return backgroundSessions.refreshOffscreenCodexHistoryAfterCompletion(backend, threadId, sourceModel, { advanceQueue })
}

function connectBackend({ backendInfoReady = false } = {}) {
  if (state.backend === 'opencode') connectOpenCode({ backendInfoReady }).catch(showError)
  else connectAppServer()
}

async function switchBackend(backend, { selectedId } = {}) {
  if (!isSupportedBackend(backend) || backend === state.backend) return
  const previousBackend = state.backend
  captureTranscriptViewState()
  if (!leaveSessionWorkspace()) return false
  state.selectedByBackend[state.backend] = state.selectedId
  cleanupConnections()
  const transitionGeneration = state.socketGeneration
  const backgroundConnected = isCodexBackend(previousBackend)
    ? codexLifecycleConnection.ready
    : previousBackend === 'opencode' && openCodeStreamState.ready
  if (!backgroundConnected) {
    state.backendStates[previousBackend] = { kind: 'idle', label: backendDescriptor(previousBackend).name, caption: 'Connect on demand' }
  }
  rejectPending(new Error('Backend switched'))
  state.backend = backend
  if (selectedId) state.selectedByBackend[backend] = selectedId
  state.selectedId = state.selectedByBackend[backend] || null
  sessionMap.resetSelection()
  state.threads = state.threadsByBackend[backend]
  const cached = cachedThreadModel(backend, state.selectedId)
  state.model = cached?.model || createCodexViewModel()
  if (state.selectedId) state.model.threadId = state.selectedId
  prepareTranscriptViewForSelection({
    historyReady: Boolean(cached),
    historyComplete: cached?.model?.historyComplete !== false,
  })
  state.backendInfo = state.backendInfos[backend]
  state.ready = false
  applyBackendCopy()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  sessionMap.render()
  persistPreferences()
  await loadBackendInfo(backend)
  if (state.backend !== backend || state.socketGeneration !== transitionGeneration) return
  connectBackend({ backendInfoReady: true })
}

function applyBackendCopy() {
  const descriptor = currentBackend()
  applyRuntimeCopy()
  $('#empty-mark').textContent = descriptor.tag.slice(0, 1)
  $('#tool-avatar').textContent = descriptor.tag
  $('#new-thread-label').textContent = t('New session')
  $('#native-error-title').textContent = t('{backend} Server unavailable', { backend: descriptor.name })
  $('#empty-title').textContent = t('Structured {backend} workspace', { backend: descriptor.name })
  $('#empty-description').textContent = isCodexBackend(descriptor.id)
    ? 'Messages, commands, file changes, plans, approvals, and stop reasons come from this independent Codex App Server instance.'
    : 'Messages, tools, file changes, permissions, and stop reasons come directly from OpenCode Server as structured events.'
  $('#composer-input').placeholder = t('Message {backend}… @ files · $ skills · / commands · ! shell', { backend: descriptor.name })
  $('#rename-thread-description').textContent = t('The name is persisted by {backend}.', { backend: descriptor.name })
  const wsl = state.backendInfo?.executionEnvironment === 'wsl'
  $('#new-thread-cwd').placeholder = '/home/user/projects/project'
  $('#new-thread-cwd-help').textContent = t(wsl
    ? 'Enter an absolute Linux path inside WSL, such as /home/user/project.'
    : 'Must be an absolute local path.')
}

function applyRuntimeCopy() {
  const title = state.remoteClient ? t('Remote workspace') : t('Local workspace')
  const detail = state.remoteClient ? t('SSH tunnel') : title
  const copy = $('.studio-entry-copy')
  if (copy) {
    copy.querySelector('strong').textContent = title
    copy.querySelector('small').textContent = detail
  }
}

function connectAppServer() {
  const descriptor = currentBackend()
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  connectSelectedSocket(state, {
    cleanup: cleanupSocket,
    starting: () => {
      setBackendState('checking', `Starting ${descriptor.name}`, 'App Server · stdio')
      setNativeError(null)
      renderComposerState()
    },
    open: () => gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`),
    message: (data) => {
      try { handleAppServerMessage(JSON.parse(data)) }
      catch (error) { console.error('Invalid App Server message', error, data) }
    },
    error: () => {
      setBackendState('error', `${descriptor.name} Disconnected`, 'WebSocket connection failed')
      renderComposerState()
    },
    closed: () => {
      rejectPending(new Error(`${descriptor.name} App Server connection closed`))
      setBackendState('error', t('{backend} disconnected', { backend: descriptor.name }), 'Preparing to reconnect…')
      setNativeError(t('The connection to the local {backend} App Server closed.', { backend: descriptor.name }))
      renderComposerState()
    },
    reconnect: connectAppServer,
  })
}

async function connectOpenCode({ backendInfoReady = false } = {}) {
  clearTimeout(state.reconnectTimer)
  cleanupConnections()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  setBackendState('checking', 'Starting OpenCode', 'Server · HTTP/SSE')
  setNativeError(null)
  renderComposerState()
  if (!backendInfoReady) await loadBackendInfo()
  if (generation !== state.socketGeneration) return
  if (state.backendInfo?.reachable === false || state.backendInfo?.error) {
    const reason = state.backendInfo.error || 'OpenCode Server is not ready'
    setBackendState('error', 'OpenCode unavailable', reason)
    setNativeError(reason)
    return
  }
  state.ready = true
  renderComposerState()
  loadBackendModels().catch((error) => console.debug('Unable to load OpenCode models', error))
  startOpenCodeEventStream()
  // Prefer establishing the event barrier before taking the history snapshot.
  // If the stream is slow to open, the late first onopen advances the epoch and
  // schedules a second authoritative read so the gap still cannot be hidden.
  const eventStreamReady = await waitForOpenCodeEventStream()
  if (generation !== state.socketGeneration || state.backend !== 'opencode') return
  if (!eventStreamReady) openCodeStreamState.missedBarrier = true
  renderOpenCodeConnectionState(
    eventStreamReady ? 'online' : 'checking',
    eventStreamReady ? 'OpenCode Server' : 'Connecting to OpenCode events',
    eventStreamReady ? 'Native structured connection' : 'HTTP ready · SSE event stream',
  )
  const selectionLoad = loadThreads()
  backendSelectionLoads.set('opencode', selectionLoad)
  try {
    await selectionLoad
  } catch (error) {
    handleThreadCatalogFailure('opencode', generation, error)
  } finally {
    if (backendSelectionLoads.get('opencode') === selectionLoad) backendSelectionLoads.delete('opencode')
  }
}

function cleanupSocket() {
  if (!state.socket) return
  state.socket.onclose = null
  state.socket.close()
  state.socket = null
}

function cleanupConnections() {
  clearTimeout(state.reconnectTimer)
  clearTimeout(threadCatalogRetryTimer)
  threadCatalogRetryTimer = null
  threadCatalogRetryAttempt = 0
  threadCatalogErrorMessage = null
  cleanupSocket()
  state.socketGeneration += 1
}

function beginTurnLatencyTrace(clientId, threadId) {
  const trace = {
    clientId: String(clientId || ''),
    threadId: String(threadId || ''),
    turnId: '',
    startedAt: performance.now(),
    marks: new Set(),
  }
  turnLatencyTraces.set(`client:${trace.clientId}`, trace)
  console.info('[Codex latency] send', { clientId: trace.clientId, threadId: trace.threadId })
  return trace
}

function markTurnLatency(trace, phase) {
  if (!trace || trace.marks.has(phase)) return
  trace.marks.add(phase)
  console.info(`[Codex latency] ${phase}`, {
    elapsedMs: Math.round(performance.now() - trace.startedAt),
    threadId: trace.threadId,
    turnId: trace.turnId || undefined,
  })
}

function bindTurnLatencyTrace(trace, turnId) {
  if (!trace || !turnId) return trace
  trace.turnId = String(turnId)
  turnLatencyTraces.set(`turn:${trace.turnId}`, trace)
  return trace
}

function pendingTurnLatencyTrace(threadId) {
  const target = String(threadId || '')
  return [...new Set(turnLatencyTraces.values())]
    .find((trace) => !trace.turnId && trace.threadId === target) || null
}

function finishTurnLatencyTrace(trace, phase = 'complete') {
  if (!trace) return
  markTurnLatency(trace, phase)
  turnLatencyTraces.delete(`client:${trace.clientId}`)
  if (trace.turnId) turnLatencyTraces.delete(`turn:${trace.turnId}`)
}

function observeCodexTurnLatency(message, backend = state.backend) {
  const params = message?.params || {}
  const turnId = params.turnId || params.turn?.id
  const threadId = params.threadId || params.thread?.id || (state.backend === backend ? state.selectedId : null)
  let trace = turnId ? turnLatencyTraces.get(`turn:${turnId}`) : null
  if (!trace && message?.method === 'turn/started') {
    trace = bindTurnLatencyTrace(pendingTurnLatencyTrace(threadId), turnId)
  }
  if (!trace) return
  if (message.method === 'turn/started') markTurnLatency(trace, 'turn_started')
  else if (message.method === 'turn/completed') finishTurnLatencyTrace(trace)
  else if (message.method?.startsWith('item/') || message.method === 'turn/plan/updated') {
    markTurnLatency(trace, 'first_activity')
    requestAnimationFrame(() => markTurnLatency(trace, 'first_ui_paint'))
  }
}

function claimCodexLifecycleNotification(backend, message) {
  return claimLifecycleNotification(backend, message,
    handledCodexTurnLifecycleEvents, recentCodexStatusLifecycleEvents, () => performance.now())
}

function handleCodexLifecycleNotification(backend, message) {
  return backgroundSessions.handleCodexLifecycleNotification(backend, message)
}

function handleAppServerMessage(...args) {
  return activeCodexConnection.handleAppServerMessage(...args)
}

async function resynchronizeSelectedThreadAfterLag(...args) {
  return activeCodexConnection.resynchronizeSelectedThreadAfterLag(...args)
}

async function captureOffscreenInteraction(...args) {
  return activeCodexConnection.captureOffscreenInteraction(...args)
}

function notifyDesktop(title, body = '', ref = null) {
  if (!state.desktopNotifications || (!document.hidden && document.hasFocus())) return
  if (!('Notification' in window) || Notification.permission !== 'granted') return
  try {
    const notification = new Notification(String(title || 'Codex Thread Studio'), {
      body: String(body || '').slice(0, 240),
      tag: `codex-thread-studio:${ref?.backend || state.backend}:${ref?.id || state.selectedId || 'workspace'}`,
    })
    notification.onclick = () => {
      window.focus()
      notification.close()
    }
  } catch (error) {
    console.warn('Desktop notification failed', error)
  }
}

function beginOpenCodeHistoryEventBuffer(threadId) {
  const key = String(threadId || '')
  const buffer = []
  if (!key) return buffer
  let buffers = openCodeHistoryEventBuffers.get(key)
  if (!buffers) {
    buffers = new Set()
    openCodeHistoryEventBuffers.set(key, buffers)
  }
  buffers.add(buffer)
  return buffer
}

function endOpenCodeHistoryEventBuffer(threadId, buffer) {
  const key = String(threadId || '')
  const buffers = openCodeHistoryEventBuffers.get(key)
  if (!buffers) return
  buffers.delete(buffer)
  if (!buffers.size) openCodeHistoryEventBuffers.delete(key)
}

function bufferOpenCodeHistoryEvent(threadId, event) {
  const key = String(threadId || '')
  if (!key) return
  const sequence = (openCodeHistoryEventSequences.get(key) || 0) + 1
  openCodeHistoryEventSequences.set(key, sequence)
  for (const buffer of openCodeHistoryEventBuffers.get(key) || []) buffer.push({ event, sequence })
}

function openCodeHistoryIncludesDeletion(buffer, threadId) {
  return (buffer || []).some(({ event }) => {
    const payload = event?.payload || event
    return payload?.type === 'session.deleted' && String(openCodeEventThreadId(payload) || '') === String(threadId || '')
  })
}

function handleOpenCodeServerEvent(event) {
  return backgroundSessions.handleOpenCodeServerEvent(event)
}

async function abortRepeatedOpenCodeTerminalLoop({ sessionId, parentId, assistantIds }) {
  if (!sessionId || openCodeLoopAbortRequests.has(sessionId)) return
  const thread = state.threadsByBackend.opencode.find((candidate) => candidate.id === sessionId)
  openCodeLoopAbortRequests.add(sessionId)
  console.warn('Stopping repeated OpenCode terminal-assistant loop', { sessionId, parentId, assistantIds })
  try {
    let directory = thread?.cwd || ''
    if (!directory) {
      const session = await openCodeFetch(`/session/${encodeURIComponent(sessionId)}`, {
        timeoutMs: 10_000,
        allowInactive: true,
      })
      directory = session?.directory || ''
      if (!directory) throw new Error('OpenCode did not return the looping session directory')
    }
    await openCodeFetch(withDirectory(`/session/${encodeURIComponent(sessionId)}/abort`, directory), {
      method: 'POST',
      timeoutMs: 10_000,
      allowInactive: true,
    })
    if (state.backend === 'opencode' && state.selectedId === sessionId) {
      toast(t('Stopped an abnormal OpenCode response loop after repeated final replies.'), 'error')
    }
  } catch (error) {
    console.warn('Unable to stop repeated OpenCode response loop', error)
  } finally {
    openCodeLoopAbortRequests.delete(sessionId)
  }
}

function openCodeCompletionSignal(payload) {
  if (payload?.type === 'message.part.updated') return payload.properties?.part?.type === 'step-finish'
  if (payload?.type !== 'message.updated') return false
  const info = payload.properties?.info
  return info?.role === 'assistant' && Boolean(info.time?.completed)
}

function scheduleOpenCodeStatusReconciliation(threadId) {
  clearTimeout(openCodeStatusReconcileTimers.get(threadId))
  const timer = setTimeout(async () => {
    openCodeStatusReconcileTimers.delete(threadId)
    const thread = state.threadsByBackend.opencode.find((candidate) => candidate.id === threadId)
    if (!thread) return
    try {
      const statuses = await openCodeFetch(withDirectory('/session/status', thread.cwd), {
        timeoutMs: 10_000,
        allowInactive: true,
      })
      handleOpenCodeServerEvent({
        type: 'session.status',
        properties: { sessionID: threadId, status: statuses?.[threadId] || { type: 'idle' } },
      })
    } catch (error) {
      console.debug('Unable to reconcile OpenCode session status', error)
    }
  }, 250)
  openCodeStatusReconcileTimers.set(threadId, timer)
}

function scheduleOpenCodeListRefresh({ forceSelectedHistory = false } = {}) {
  openCodeListRefreshNeedsHistory ||= forceSelectedHistory
  clearTimeout(openCodeListRefreshTimer)
  openCodeListRefreshTimer = setTimeout(() => {
    const refreshSelectedHistory = openCodeListRefreshNeedsHistory
    openCodeListRefreshNeedsHistory = false
    const refresh = state.backend === 'opencode' && state.ready
      ? refreshOpenCodeThreadList({ forceSelectedHistory: refreshSelectedHistory })
      : refreshBackendCatalog('opencode')
    refresh.catch(console.error)
  }, 180)
}

async function refreshOpenCodeThreadList({ forceSelectedHistory = false } = {}) {
  if (state.backend !== 'opencode' || !state.ready) return
  const initialSelection = backendSelectionLoads.get('opencode')
  if (initialSelection) await initialSelection
  if (state.backend !== 'opencode' || !state.ready) return
  const socketGeneration = state.socketGeneration
  const selectedId = state.selectedId
  const catalogGeneration = (catalogRequestGenerations.get('opencode') || 0) + 1
  catalogRequestGenerations.set('opencode', catalogGeneration)
  const result = await rpc('thread/list', { limit: 100 })
  if (state.backend !== 'opencode'
    || state.socketGeneration !== socketGeneration
    || catalogRequestGenerations.get('opencode') !== catalogGeneration
    || !state.ready) return
  setActiveThreads(Array.isArray(result?.data) ? result.data : [], 'opencode')
  renderThreadList()
  renderWorkspace()
  if (state.selectedId !== selectedId) return
  if (selectedId && (forceSelectedHistory || !freshThreadModel('opencode', selectedId))) {
    const activeHistory = state.threadLoads.get(threadCatalogKey('opencode', selectedId))
    if (activeHistory) await activeHistory
    if (state.backend !== 'opencode'
      || state.socketGeneration !== socketGeneration
      || state.selectedId !== selectedId
      || !state.ready) return
  }
  if (selectedId && (forceSelectedHistory || !freshThreadModel('opencode', selectedId))) {
    await refreshSelectedThread({ quiet: true })
  }
}

function rpc(method, params = {}, timeoutMs = 30_000, supervised = false) {
  if (!supervised && params.threadId) turnSupervision.humanAction({ backend: state.backend, id: params.threadId }, method, params)
  if (state.backend === 'opencode') return openCodeRpc(method, params, timeoutMs)
  if (!state.ready || state.socket?.readyState !== WebSocket.OPEN) return Promise.reject(new Error(t('{backend} App Server is not ready', { backend: currentBackend().name })))
  const id = ++state.requestId
  return pendingRpcRequests.request(id, method, () => sendRaw({ id, method, params }), timeoutMs, t('{method} request timed out', { method }))
}

async function dispatchBackendRpc(backend, method, params = {}, timeoutMs = 30_000, supervised = false) {
  if (!supervised && params.threadId) turnSupervision.humanAction({ backend, id: params.threadId }, method, params)
  if (backend === state.backend && state.ready) return rpc(method, params, timeoutMs, true)
  if (isCodexBackend(backend)) return codexBackgroundRpc(backend, method, params, timeoutMs)
  if (backend === 'opencode') {
    await ensureOpenCodeAvailable()
    return openCodeRpc(method, params, timeoutMs, { allowInactive: true })
  }
  throw new Error(`Unsupported session backend: ${backend}`)
}

function codexBackgroundRpc(backend, method, params = {}, timeoutMs = 30_000) {
  const descriptor = backendDescriptor(backend)
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
  return requestSocketRpc({
    socket, id: -(Date.now() + Math.floor(Math.random() * 100_000)), method, params, timeoutMs,
    timeoutMessage: t('{method} request timed out', { method }),
    unavailableMessage: 'Codex App Server is unavailable',
    connectMessage: t('Unable to connect to the {backend} App Server', { backend: descriptor.name }),
    closeMessage: t('The {backend} App Server connection closed', { backend: descriptor.name }),
    responseError: (error) => error.message || JSON.stringify(error),
  })
}

async function ensureOpenCodeAvailable(...args) {
  return openCodeProtocol.ensureOpenCodeAvailable(...args)
}

async function openCodeFetch(...args) {
  return openCodeProtocol.openCodeFetch(...args)
}

async function fetchOpenCodeMessageHistory(...args) {
  return openCodeProtocol.fetchOpenCodeMessageHistory(...args)
}

async function fetchOpenCodeStatusSnapshot(...args) {
  return openCodeProtocol.fetchOpenCodeStatusSnapshot(...args)
}

async function fetchOpenCodeCatalog(...args) {
  return openCodeProtocol.fetchOpenCodeCatalog(...args)
}

function fetchCodexCatalog(backend, limit = 100, { routerId = null, routerWorkspace = '' } = {}) {
  return new Promise((resolve, reject) => {
    const descriptor = backendDescriptor(backend)
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
    const id = -(Date.now() + Math.floor(Math.random() * 100_000))
    const routerReadId = id - 1
    const timer = setTimeout(() => finish(new Error('Codex session catalog request timed out')), 15_000)
    let requested = false
    let catalog = []
    const finish = (error, value) => {
      clearTimeout(timer)
      socket.onclose = null
      socket.close()
      if (error) reject(error)
      else resolve(Array.isArray(value?.data) ? value.data : [])
    }
    socket.onmessage = (event) => {
      let message
      try { message = JSON.parse(event.data) } catch { return }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'error') {
        finish(new Error(message.params?.message || 'Codex App Server is unavailable'))
        return
      }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'ready' && !requested) {
        requested = true
        socket.send(JSON.stringify({ id, method: 'thread/list', params: catalogListParams('codex', { limit }) }))
        return
      }
      if (message.id === id) {
        if (message.error) {
          finish(new Error(message.error.message || 'Codex session catalog request failed'))
          return
        }
        catalog = Array.isArray(message.result?.data) ? message.result.data : []
        if (!routerId || managedRouterThread(catalog, routerId, routerWorkspace)) {
          finish(null, { data: catalog })
          return
        }
        socket.send(JSON.stringify({ id: routerReadId, method: 'thread/read', params: { threadId: routerId, includeTurns: false } }))
        return
      }
      if (message.id === routerReadId) {
        const recovered = message.error ? null : message.result?.thread
        finish(null, { data: recoverManagedRouterCatalog(catalog, routerId, routerWorkspace, recovered) })
      }
    }
    socket.onerror = () => finish(new Error(t('Unable to read the {backend} session catalog', { backend: descriptor.name })))
  })
}

function requestCodexBackend(backend, method, params = {}, timeoutMs = 15_000) {
  const descriptor = backendDescriptor(backend)
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
  return requestSocketRpc({
    socket, id: -(Date.now() + Math.floor(Math.random() * 100_000)), method, params, timeoutMs,
    timeoutMessage: t('{backend} request timed out', { backend: descriptor.name }),
    unavailableMessage: t('{backend} App Server is unavailable', { backend: descriptor.name }),
    connectMessage: t('Unable to connect to the {backend} App Server', { backend: descriptor.name }),
    closeMessage: t('The {backend} App Server connection closed', { backend: descriptor.name }),
    responseError: (error) => error.message || `${method} failed`,
  })
}

async function fetchBackendCatalog(backend, { includeStatuses = true } = {}) {
  if (backend === 'opencode') {
    const threads = await fetchOpenCodeCatalog(100, { allowInactive: true, includeStatuses })
    if (includeStatuses) return threads
    const knownStatuses = new Map((state.threadsByBackend.opencode || []).map((thread) => [thread.id, thread.status]))
    return threads.map((thread) => knownStatuses.has(thread.id)
      ? { ...thread, status: knownStatuses.get(thread.id) }
      : thread)
  }
  if (isCodexBackend(backend)) {
    const info = await loadBackendInfo(backend)
    return fetchCodexCatalog(backend, 100, {
      routerId: state.router.controllers[backend] || null,
      routerWorkspace: info?.routerWorkspace || '',
    })
  }
  throw new Error(t('Unsupported session backend: {backend}', { backend }))
}

async function refreshBackendCatalog(backend, { includeStatuses = true } = {}) {
  const activeSelection = backend === state.backend ? backendSelectionLoads.get(backend) : null
  if (activeSelection) {
    await activeSelection
    return state.threadsByBackend[backend]
  }
  const refreshKey = `${backend}:${includeStatuses ? 'full' : 'list'}`
  if (catalogRefreshes.has(refreshKey)) return catalogRefreshes.get(refreshKey)
  const generation = (catalogRequestGenerations.get(backend) || 0) + 1
  catalogRequestGenerations.set(backend, generation)
  const refresh = fetchBackendCatalog(backend, { includeStatuses })
    .then((threads) => {
      if (catalogRequestGenerations.get(backend) === generation) installBackendCatalog(backend, threads)
      return threads
    })
    .finally(() => catalogRefreshes.delete(refreshKey))
  catalogRefreshes.set(refreshKey, refresh)
  return refresh
}

function scheduleCodexCatalogRecovery(backend) {
  if (!isCodexBackend(backend) || codexCatalogRecoveryStarted.has(backend)) return
  const catalog = state.threadsByBackend[backend]
  const preferredId = state.selectedByBackend[backend]
  if (!shouldRecoverCodexCatalog(catalog, preferredId)) return
  codexCatalogRecoveryStarted.add(backend)
  scheduleStudioIdleWork(async () => {
    const currentCatalog = state.threadsByBackend[backend]
    const currentPreferredId = state.selectedByBackend[backend]
    if (!shouldRecoverCodexCatalog(currentCatalog, currentPreferredId)) {
      codexCatalogRecoveryStarted.delete(backend)
      return
    }
    if ([...state.threadLoads.keys()].some((key) => key.startsWith(`${backend}:`))) {
      codexCatalogRecoveryStarted.delete(backend)
      scheduleCodexCatalogRecovery(backend)
      return
    }
    if (currentCatalog.length && currentPreferredId && !currentCatalog.some((thread) => thread.id === currentPreferredId)) {
      try {
        const result = await dispatchBackendRpc(backend, 'thread/read', { threadId: currentPreferredId, includeTurns: false })
        if (result?.thread) {
          mergeThreadIntoCatalog(backend, result.thread)
          renderThreadList()
          if (state.backend === backend && state.selectedId === currentPreferredId) {
            renderWorkspace()
            renderComposerState()
            if (!freshThreadModel(backend, currentPreferredId)) {
              await refreshSelectedThread({ quiet: true })
            }
          }
        }
      } catch (error) {
        if (/not found|does not exist|unknown thread/iu.test(String(error?.message || error))) {
          console.debug(`Saved ${backend} session is no longer available`, error)
        } else {
          codexCatalogRecoveryStarted.delete(backend)
          console.debug(`Unable to verify saved ${backend} session; recovery will retry`, error)
          scheduleCodexCatalogRecovery(backend)
        }
      }
      return
    }
    try {
      // The default list path scans rollout files and repairs the state index.
      // Its response may contain creation-time metadata, so never install it.
      await dispatchBackendRpc(backend, 'thread/list', { limit: 100 })
      await refreshBackendCatalog(backend)
    } catch (error) {
      codexCatalogRecoveryStarted.delete(backend)
      console.warn(`Unable to recover ${backend} session catalog`, error)
    }
  })
}

function scheduleStudioIdleWork(callback, { delay = 1_200, timeout = 4_000 } = {}) {
  setTimeout(() => {
    if (typeof globalThis.requestIdleCallback === 'function') {
      globalThis.requestIdleCallback(() => callback(), { timeout })
    } else {
      callback()
    }
  }, delay)
}

function scheduleInactiveCatalogRefresh() {
  if (inactiveCatalogRefreshScheduled) return
  inactiveCatalogRefreshScheduled = true
  scheduleStudioIdleWork(async () => {
    inactiveCatalogRefreshScheduled = false
    await refreshInactiveCatalog()
  })
}

async function refreshActiveCodexCatalogOnFocus() {
  const backend = state.backend
  if (!state.ready || !isCodexBackend(backend)) return
  const now = Date.now()
  if (now - codexCatalogFocusRefreshAt < 2_000) return
  codexCatalogFocusRefreshAt = now
  const selectedId = state.selectedId
  const previousCwd = threadForRef({ backend, id: selectedId })?.cwd || ''
  try {
    await refreshBackendCatalog(backend)
    scheduleCodexCatalogRecovery(backend)
    if (state.backend !== backend || state.selectedId !== selectedId) return
    const currentCwd = threadForRef({ backend, id: selectedId })?.cwd || ''
    if (currentCwd !== previousCwd) {
      renderWorkspace()
      renderComposerState()
    }
  } catch (error) {
    console.debug(`Unable to refresh ${backend} catalog after focus`, error)
  }
}

function reportSessionLifecycle(phase, details = {}) {
  const bounded = {}
  for (const [key, value] of Object.entries(details)) {
    if (typeof value === 'string') bounded[key] = value.slice(0, 256)
    else if (typeof value === 'number' && Number.isFinite(value)) bounded[key] = value
    else if (typeof value === 'boolean' || value == null) bounded[key] = value
  }
  const message = `[session.lifecycle] ${phase} ${JSON.stringify(bounded)}`
  console.info(message)
  gatewayFetch('/studio/client-log', {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: message,
  }).catch(() => {})
}

function lifecycleStatusValue(value) {
  if (typeof value === 'string') return value
  return typeof value?.type === 'string' ? value.type : ''
}

function codexLifecycleThreadId(backend, event, model = null) {
  return event?.threadId
    || threadIdForCachedModel(backend, model)
    || (state.backend === backend ? state.selectedId : '')
}

function reportCodexLifecycleNotification(backend, message, {
  targetModel = null,
  applied = false,
  beforeModelStatus = '',
  beforeActiveTurnId = '',
} = {}) {
  const event = codexLifecycleEvent(message)
  if (!event) return
  const threadId = codexLifecycleThreadId(backend, event, targetModel)
  const cache = cachedThreadModel(backend, threadId)
  const thread = (state.threadsByBackend[backend] || []).find((candidate) => candidate.id === threadId)
  reportSessionLifecycle('codex-notification', {
    backend,
    method: event.method,
    threadId,
    turnId: event.turnId,
    notificationStatus: event.notificationStatus,
    selectedId: state.backend === backend ? state.selectedId : '',
    offscreen: Boolean(threadId && (state.backend !== backend || state.selectedId !== threadId)),
    cacheAvailable: Boolean(cache),
    routed: Boolean(targetModel),
    routedToCache: Boolean(cache && targetModel && cache.model === targetModel),
    applied: Boolean(applied),
    beforeModelStatus,
    beforeActiveTurnId,
    modelStatus: targetModel?.status || '',
    activeTurnId: targetModel?.activeTurnId || '',
    catalogStatus: lifecycleStatusValue(thread?.status),
    catalogUpdatedAt: threadUpdatedAt(thread),
    catalogActivityAt: catalogActivityTimestamp(thread),
    cacheValidatedAt: Number(cache?.validatedAt || 0),
    socketGeneration: state.socketGeneration,
  })
}

function reportCodexSelectionCacheDecision(backend, id, { fresh = null, cached = null } = {}) {
  if (!isCodexBackend(backend)) return
  const thread = (state.threadsByBackend[backend] || []).find((candidate) => candidate.id === id)
  reportSessionLifecycle('codex-selection-cache', {
    backend,
    threadId: id,
    decision: fresh ? 'restore-fresh' : cached ? 'show-stale-and-revalidate' : 'load-history',
    cacheAvailable: Boolean(cached),
    modelStatus: cached?.model?.status || '',
    activeTurnId: cached?.model?.activeTurnId || '',
    catalogStatus: lifecycleStatusValue(thread?.status),
    catalogUpdatedAt: threadUpdatedAt(thread),
    catalogActivityAt: catalogActivityTimestamp(thread),
    cacheValidatedAt: Number(cached?.validatedAt || 0),
    socketGeneration: state.socketGeneration,
  })
}

function rememberStartedThread(backend, thread, operation) {
  startedSessionCatalog.remember(backend, thread, operation)
}

function forgetStartedThread(backend, threadId, reason) {
  return startedSessionCatalog.forget(backend, threadId, reason)
}

function reconcileCatalogWithStartedThreads(backend, threads) {
  return startedSessionCatalog.reconcile(backend, threads)
}

function scheduleStartedThreadCatalogConfirmation(backend, threadId, delay = 800) {
  startedSessionCatalog.schedule(backend, threadId, delay)
}

function clearStartedThreadsForBackend(backend, reason) {
  startedSessionCatalog.clearBackend(backend, reason)
}

function installBackendCatalog(backend, threads) {
  const visible = preserveCatalogActivity(
    reconcileCatalogWithStartedThreads(backend, threads),
    state.threadsByBackend[backend],
  )
    .filter((thread) => !hiddenUtilityThread(backend, thread))
  state.threadsByBackend[backend] = visible
  if (backend === state.backend) state.threads = visible
  renderThreadList()
}

async function refreshRouterCatalogs() {
  try {
    const backends = sessionDispatch.backends()
    const catalogs = await Promise.all(backends.map((backend) =>
      refreshBackendCatalog(backend, { includeStatuses: false }),
    ))
    backends.forEach((backend, index) => {
      catalogRequestGenerations.set(backend, (catalogRequestGenerations.get(backend) || 0) + 1)
      installBackendCatalog(backend, catalogs[index])
    })
  } catch (error) {
    throw new Error(t('Unable to refresh the complete session catalog: {message}', { message: error.message }))
  }
}

async function refreshInactiveCatalog() {
  await Promise.all(BACKEND_IDS.filter((backend) => backend !== state.backend).map(async (backend) => {
    try {
      await refreshBackendCatalog(backend)
      scheduleCodexCatalogRecovery(backend)
      if (backend === state.router.controllerBackend) await threadRouter.ensureManagedSession(backend)
    } catch (error) {
      console.warn(`Unable to refresh ${backend} catalog`, error)
      reportClientError(new Error(`${backend} inactive session catalog failed: ${error?.message || error}`))
    }
  }))
}

function handleThreadCatalogFailure(backend, generation, error) {
  if (backend !== state.backend || generation !== state.socketGeneration || !state.ready) return
  threadCatalogRetryAttempt += 1
  threadCatalogErrorMessage = t('Unable to load the {backend} session list. Studio will retry automatically.', {
    backend: backendDescriptor(backend).name,
  })
  console.warn(`${backend} session catalog load failed`, error)
  reportClientError(new Error(`${backend} session catalog load failed: ${error?.message || error}`))
  if (threadCatalogRetryAttempt === 1) {
    setNativeError(threadCatalogErrorMessage)
    toast(threadCatalogErrorMessage, 'error')
  }
  clearTimeout(threadCatalogRetryTimer)
  const delay = Math.min(1_500 * (2 ** (threadCatalogRetryAttempt - 1)), 15_000)
  threadCatalogRetryTimer = setTimeout(() => {
    threadCatalogRetryTimer = null
    if (backend !== state.backend || generation !== state.socketGeneration || !state.ready) return
    loadThreads().catch((retryError) => handleThreadCatalogFailure(backend, generation, retryError))
  }, delay)
}

function markThreadCatalogLoaded() {
  clearTimeout(threadCatalogRetryTimer)
  threadCatalogRetryTimer = null
  threadCatalogRetryAttempt = 0
  if (threadCatalogErrorMessage && $('#native-error-message').textContent === threadCatalogErrorMessage) {
    setNativeError(null)
  }
  threadCatalogErrorMessage = null
}

function directoryQuery(...args) {
  return openCodeProtocol.directoryQuery(...args)
}

function withDirectory(...args) {
  return openCodeProtocol.withDirectory(...args)
}

async function openCodeRpc(...args) {
  return openCodeProtocol.openCodeRpc(...args)
}

function openCodeFilePart(...args) {
  return openCodeProtocol.openCodeFilePart(...args)
}

function openCodeTurnResult(...args) {
  return openCodeProtocol.openCodeTurnResult(...args)
}

async function openCodeUserMessageBaseline(...args) {
  return openCodeProtocol.openCodeUserMessageBaseline(...args)
}

async function openCodeStartedTurn(...args) {
  return openCodeProtocol.openCodeStartedTurn(...args)
}

function sendRaw(message) {
  if (state.socket?.readyState !== WebSocket.OPEN) throw new Error('Codex App Server connection is not open')
  state.socket.send(JSON.stringify(message))
}

function rejectPending(error) {
  pendingRpcRequests.rejectAll(error)
}

async function loadThreads({ applyCachedEnvironment = true } = {}) {
  const backend = state.backend
  const socketGeneration = state.socketGeneration
  const archiveOpen = sessionManagement.archive.isOpen()
  if (state.startupRouterSelectionPending && !archiveOpen) {
    const routerId = await threadRouter.ensureManagedSession(backend)
    if (state.backend !== backend || state.socketGeneration !== socketGeneration || !state.ready) return false
    state.selectedByBackend[backend] = routerId
    state.selectedId = routerId
    state.startupRouterSelectionPending = false
  }
  const preferred = state.selectedId
  const preferredIsRouter = Boolean(preferred) && isRouterThread(preferred, backend)
  const knownPreferred = !archiveOpen && preferred
    ? state.threadsByBackend[backend].find((thread) => thread.id === preferred)
    : null
  const preferredVisible = knownPreferred
    && (preferredIsRouter
      || !isSessionDirectoryHidden(knownPreferred.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore))
  let preferredLoad = null
  let preferredUsedCache = false
  let preferredVerified = false
  if (preferredVisible) {
    const cached = freshThreadModel(backend, preferred)
    preferredUsedCache = Boolean(cached)
    preferredLoad = cached
      ? loadSelectedSessionCompanions(backend, preferred, { applyEnvironment: applyCachedEnvironment })
      : selectThread(preferred, { force: true, backend }).catch(showError)
  }
  // Invalidate any inactive-catalog response that started before this backend
  // became active, but reuse its network work when possible. The preferred
  // history above starts immediately and does not wait for sidebar metadata.
  const existingCatalogLoad = catalogRefreshes.get(`${backend}:full`)
    || catalogRefreshes.get(`${backend}:list`)
  const catalogGeneration = (catalogRequestGenerations.get(backend) || 0) + 1
  catalogRequestGenerations.set(backend, catalogGeneration)
  const result = existingCatalogLoad
    ? { data: await existingCatalogLoad }
    : await dispatchBackendRpc(backend, 'thread/list', catalogListParams(backendDescriptor(backend).kind, { limit: 100 }))
  if (state.backend !== backend
    || state.socketGeneration !== socketGeneration
    || catalogRequestGenerations.get(backend) !== catalogGeneration
    || !state.ready) return false
  const listedThreads = Array.isArray(result?.data) ? result.data : []
  const preferredMissingFromCatalog = Boolean(preferred)
    && !listedThreads.some((thread) => thread.id === preferred)
  setActiveThreads(listedThreads, backend)
  if (preferredLoad && knownPreferred && preferredMissingFromCatalog
    && (isCodexBackend(backend) || preferredIsRouter)) {
    mergeThreadIntoCatalog(backend, knownPreferred)
  }
  if (backend === 'opencode' && preferredMissingFromCatalog && !preferredIsRouter) {
    invalidateThreadModel(backend, preferred)
  }
  markThreadCatalogLoaded()
  if (backend === state.router.controllerBackend) {
    await threadRouter.ensureManagedSession(backend).catch(showError)
    if (state.backend !== backend || state.socketGeneration !== socketGeneration || !state.ready) return false
  }
  renderThreadList()
  // Catalog requests are deliberately parallel with the selected history.
  // If the operator chose another session while this request was in flight,
  // install the new sidebar metadata but never replay the old auto-selection.
  if (sessionManagement.archive.isOpen() !== archiveOpen || state.selectedId !== preferred) {
    scheduleCodexCatalogRecovery(backend)
    scheduleInactiveCatalogRefresh()
    return true
  }
  if (archiveOpen) {
    renderWorkspace()
    scheduleCodexCatalogRecovery(backend)
    scheduleInactiveCatalogRefresh()
    return true
  }
  if (!preferredLoad && preferred && preferredMissingFromCatalog && isCodexBackend(backend)) {
    try {
      const result = await dispatchBackendRpc(backend, 'thread/read', { threadId: preferred, includeTurns: false })
      if (state.backend !== backend
        || state.socketGeneration !== socketGeneration
        || catalogRequestGenerations.get(backend) !== catalogGeneration
        || sessionManagement.archive.isOpen() !== archiveOpen
        || state.selectedId !== preferred
        || !state.ready) return false
      const recovered = result?.thread
      if (recovered && !isSessionDirectoryHidden(recovered.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore)) {
        mergeThreadIntoCatalog(backend, recovered)
        renderThreadList()
        preferredVerified = true
        preferredLoad = selectThread(preferred, { force: true, backend }).catch(showError)
      }
    } catch (error) {
      if (state.backend !== backend
        || state.socketGeneration !== socketGeneration
        || catalogRequestGenerations.get(backend) !== catalogGeneration
        || sessionManagement.archive.isOpen() !== archiveOpen
        || state.selectedId !== preferred
        || !state.ready) return false
      if (/not found|does not exist|unknown thread/iu.test(String(error?.message || error))) {
        console.debug(`Saved ${backend} session is no longer available`, error)
      } else {
        showError(error)
        // Preserve the saved selection on transient errors so a later catalog
        // recovery can retry it instead of silently replacing it with recent.
        preferredLoad = Promise.resolve()
      }
    }
  }
  if (state.backend !== backend
    || state.socketGeneration !== socketGeneration
    || catalogRequestGenerations.get(backend) !== catalogGeneration
    || sessionManagement.archive.isOpen() !== archiveOpen
    || state.selectedId !== preferred
    || !state.ready) return false
  const visibleThreads = sidebarThreadsForBackend(backend).filter((thread) => !isSessionDirectoryHidden(thread.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore))
  const recent = [...visibleThreads].sort((left, right) => threadUpdatedAt(right) - threadUpdatedAt(left))[0]
  // A bounded fresh catalog can temporarily omit a known selected session.
  // Its already-started history/cache load remains authoritative; keep that
  // selection so targeted catalog recovery can restore its metadata later.
  const preserveMissingPreferred = preferredLoad
    && (!preferredMissingFromCatalog || isCodexBackend(backend) || preferredIsRouter)
  const nextId = preserveMissingPreferred
    ? preferred
    : visibleThreads.some((thread) => thread.id === preferred) ? preferred : recent?.id
  if (backend === 'opencode' && preferredMissingFromCatalog && !nextId) {
    state.selectedId = null
    state.selectedByBackend[backend] = null
    state.model = createCodexViewModel()
    renderThreadList()
    renderWorkspace()
    renderTranscript()
  }
  if (nextId === preferred && preferredLoad) {
    await preferredLoad
    if (state.backend !== backend
      || state.socketGeneration !== socketGeneration
      || catalogRequestGenerations.get(backend) !== catalogGeneration
      || sessionManagement.archive.isOpen() !== archiveOpen
      || state.selectedId !== preferred
      || !state.ready) return false
    if (preferredMissingFromCatalog && isCodexBackend(backend) && !preferredVerified) {
      try {
        const result = await dispatchBackendRpc(backend, 'thread/read', { threadId: preferred, includeTurns: false })
        if (state.backend !== backend
          || state.socketGeneration !== socketGeneration
          || catalogRequestGenerations.get(backend) !== catalogGeneration
          || sessionManagement.archive.isOpen() !== archiveOpen
          || state.selectedId !== preferred
          || !state.ready) return false
        if (result?.thread) {
          mergeThreadIntoCatalog(backend, result.thread)
          renderThreadList()
          if (state.backend === backend && state.selectedId === preferred) {
            renderWorkspace()
            renderComposerState()
          }
        }
      } catch (error) {
        if (state.backend !== backend
          || state.socketGeneration !== socketGeneration
          || catalogRequestGenerations.get(backend) !== catalogGeneration
          || sessionManagement.archive.isOpen() !== archiveOpen
          || state.selectedId !== preferred
          || !state.ready) return false
        if (/not found|does not exist|unknown thread/iu.test(String(error?.message || error))) {
          state.threadsByBackend[backend] = state.threadsByBackend[backend]
            .filter((thread) => thread.id !== preferred)
          if (state.backend === backend) state.threads = state.threadsByBackend[backend]
          invalidateThreadModel(backend, preferred)
          const fallback = [...listedThreads]
            .filter((thread) => !isSessionDirectoryHidden(thread.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore))
            .sort((left, right) => threadUpdatedAt(right) - threadUpdatedAt(left))[0]
          if (fallback) await selectThread(fallback.id, { force: true, backend }).catch(showError)
          else {
            state.selectedId = null
            state.selectedByBackend[backend] = null
            state.model = createCodexViewModel()
            renderThreadList()
            renderWorkspace()
            renderTranscript()
          }
          scheduleInactiveCatalogRefresh()
          return true
        }
        showError(error)
      }
    }
    // The fresh catalog can reveal activity that the previous metadata did
    // not know about. Revalidate after the parallel companion/cache path.
    if (preferredUsedCache && !freshThreadModel(backend, nextId)) {
      await selectThread(nextId, { force: true, backend }).catch(showError)
    }
  } else if (nextId && (state.selectedId !== nextId || !freshThreadModel(backend, nextId))) {
    await selectThread(nextId, { force: true, backend }).catch(showError)
  } else {
    renderWorkspace()
    if (nextId) {
      await loadSelectedSessionCompanions(backend, nextId, { applyEnvironment: applyCachedEnvironment })
    }
  }
  if (state.backend !== backend || state.socketGeneration !== socketGeneration || !state.ready) return false
  // Catalog repair can scan every rollout. Never let it compete with the
  // selected session's initial history load.
  scheduleCodexCatalogRecovery(backend)
  scheduleInactiveCatalogRefresh()
  return true
}

function setActiveThreads(threads, backend = state.backend) {
  const visible = preserveCatalogActivity(
    reconcileCatalogWithStartedThreads(backend, threads),
    state.threadsByBackend[backend],
  )
    .filter((thread) => !hiddenUtilityThread(backend, thread))
  state.threadsByBackend[backend] = visible
  if (state.backend === backend) state.threads = visible
}

function sidebarThreadsForBackend(backend) {
  return sidebarThreadCatalogs()[backend] || []
}

function sidebarThreadCatalogs() {
  return catalogsWithSingleRouter(state.router, state.threadsByBackend)
}

function visibleThreadEntries() {
  return filterCatalogEntries(sidebarThreadCatalogs(), {
    filter: state.filter,
    search: state.search,
    attention: state.attentionThreads,
    hiddenDirectories: state.hiddenSessionDirectories,
    ignorePatterns: state.sessionDirectoryIgnore,
  })
}

function renderThreadList() {
  if (sessionManagement.archive.isOpen()) {
    sessionManagement.archive.renderList()
    return
  }
  const list = $('#thread-list')
  renderThreadCatalogCounts()
  $$('.thread-filter').forEach((button) => {
    const active = button.dataset.filter === state.filter
    button.classList.toggle('active', active)
    button.setAttribute('aria-selected', String(active))
  })
  const entries = visibleThreadEntries()
  if (!entries.length) {
    const message = state.search
      ? 'No matching sessions'
      : state.filter === 'active'
        ? 'No active sessions'
        : state.filter === 'attention'
          ? 'No loaded sessions'
        : 'No sessions yet'
    list.innerHTML = `<div class="list-empty">${t(message)}</div>`
    return
  }
  const renderRow = ({ backend, thread }) => {
    const descriptor = backendDescriptor(backend)
    const status = threadStatus(thread)
    const active = backend === state.backend && thread.id === state.selectedId
    const title = compactSidebarText(threadTitle(thread))
    const tag = descriptor.tag
    const router = backend === state.router.controllerBackend && isRouterSession(state.router, backend, thread.id)
    return `<button class="thread-row${active ? ' active' : ''}" data-thread-id="${escapeHtml(thread.id)}" data-backend="${backend}">
      <span class="status-dot ${escapeHtml(status)}"></span>
      <span class="thread-copy"><strong>${escapeHtml(title)}</strong><small data-no-i18n title="${escapeHtml(thread.cwd || t('Project directory not recorded'))}">${escapeHtml(thread.cwd || t('Project directory not recorded'))}</small></span>
      <span class="thread-tags">${state.pinnedSessions.has(threadCatalogKey(backend, thread.id)) ? `<span class="thread-pin" title="${t('Pinned')}"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 4h6l-1 5 3 3v2H7v-2l3-3zM12 14v6"/></svg></span>` : ''}${router ? '<span class="backend-tag router" title="Thread Router">RT</span>' : ''}<span class="backend-tag ${backend}" title="${descriptor.name}">${tag}</span></span>
    </button>`
  }
  const { pinnedEntries, regularEntries } = partitionPinnedCatalogEntries(
    entries,
    state.pinnedSessions,
    { order: state.filter === 'all' ? 'pin' : 'activity' },
  )
  const pinnedMarkup = pinnedEntries.length ? `<section class="thread-group pinned-thread-group">
    <div class="thread-group-heading static pinned-heading">
      <span class="thread-group-pin" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="m9 4h6l-1 5 3 3v2H7v-2l3-3zM12 14v6"/></svg></span><strong>${t('Pinned')}</strong><span>${pinnedEntries.length}</span>
    </div>
    <div class="thread-group-sessions">${pinnedEntries.map(renderRow).join('')}</div>
  </section>` : ''
  if (state.filter === 'attention') {
    list.innerHTML = `${pinnedMarkup}${regularEntries.map(renderRow).join('')}`
  } else {
    const groupedMarkup = groupCatalogEntries(regularEntries).map(({ cwd, name, entries: groupEntries }) => {
      const label = name || t('Other sessions')
      const collapsed = state.collapsedThreadGroups.has(cwd)
      return `<section class="thread-group${collapsed ? ' collapsed' : ''}" data-group-path="${escapeHtml(cwd)}">
        <button class="thread-group-heading" type="button" data-no-i18n title="${escapeHtml(cwd || t('Project directory not recorded'))}">
          <span class="twisty">▼</span><strong>${escapeHtml(label)}</strong><span>${groupEntries.length}</span>
        </button>
        <div class="thread-group-sessions">${groupEntries.map(renderRow).join('')}</div>
      </section>`
    }).join('')
    list.innerHTML = `${pinnedMarkup}${groupedMarkup}`
  }
  list.querySelectorAll('button.thread-group-heading').forEach((button) => button.addEventListener('click', () => {
    const path = button.closest('.thread-group').dataset.groupPath
    if (state.collapsedThreadGroups.has(path)) state.collapsedThreadGroups.delete(path)
    else state.collapsedThreadGroups.add(path)
    renderThreadList()
  }))
  list.querySelectorAll('.thread-row').forEach((row) => row.addEventListener('click', () =>
    selectThread(row.dataset.threadId, { backend: row.dataset.backend }),
  ))
}

function renderThreadCatalogCounts() {
  const counts = catalogCountsWithAttention(sidebarThreadCatalogs(), state.attentionThreads, state.hiddenSessionDirectories, state.sessionDirectoryIgnore)
  $('#count-all').textContent = counts.all
  $('#count-active').textContent = counts.active
  $('#count-attention').textContent = counts.attention
}

function syncThreadCatalogRow(backend, thread, previous = null) {
  if (!thread || state.search || state.filter !== 'all' || previous?.cwd !== thread.cwd) {
    renderThreadList()
    return
  }
  if (!patchThreadCatalogRow(backend, thread)) renderThreadList()
}

function patchThreadCatalogRow(backend, thread) {
  if (!thread) return false
  const row = [...$('#thread-list').querySelectorAll('.thread-row')]
    .find((candidate) => candidate.dataset.backend === backend && candidate.dataset.threadId === thread.id)
  if (!row) return false
  const status = threadStatus(thread)
  row.querySelector('.status-dot').className = `status-dot ${status}`
  row.querySelector('.thread-copy strong').textContent = threadTitle(thread)
  const path = thread.cwd || t('Project directory not recorded')
  const pathElement = row.querySelector('.thread-copy small')
  pathElement.textContent = path
  pathElement.title = path
  renderThreadCatalogCounts()
  return true
}

function syncThreadListSelection() {
  syncCatalogSelection($('#thread-list')?.querySelectorAll('.thread-row'), state.backend, state.selectedId)
}

async function loadSelectedSessionCompanions(backend, id, { applyEnvironment = true } = {}) {
  if (state.backend !== backend || state.selectedId !== id) return null
  const key = sessionMapKey(backend, id)
  const mapLoad = sessionMap.load(backend, id).catch((error) => {
    console.warn('Unable to load Session Map', error)
    if (state.backend === backend && state.selectedId === id) sessionMap.setSyncState('error', error.message)
    return null
  })
  const environmentLoad = activateSelectedEnvironment({ apply: applyEnvironment }).catch((error) => {
    reportClientError(error)
    return null
  })
  const [, profile] = await Promise.all([mapLoad, environmentLoad])
  if (state.backend !== backend || state.selectedId !== id) return profile
  sessionMap.maybeBootstrap(key, state.model)
  return profile
}

async function selectThread(id, { force = false, backend = state.backend } = {}) {
  const threadKey = threadCatalogKey(backend, id)
  const finishSelection = studioPerformance.start('thread.select', {
    backend,
    threadKey,
    force,
    sameBackend: backend === state.backend,
    cacheHit: backend === state.backend && Boolean(freshThreadModel(backend, id)),
    cacheAvailable: backend === state.backend && Boolean(cachedThreadModel(backend, id)),
  })
  try {
  if (backend !== state.backend) {
    await awaitBackendSelection({
      switchBackend: () => switchBackend(backend, { selectedId: id }),
      waitFor,
      ready: () => state.backend === backend && state.ready,
      initialLoad: () => backendSelectionLoads.get(backend),
      catalogContainsSession: () => state.threads.some(thread => thread.id === id),
      freshSelection: () => state.backend === backend && state.selectedId === id && Boolean(freshThreadModel(backend, id)),
    })
    return
  }
  if (!force && state.selectedId === id) return
  sessionManagement.search.close({ clear: true })
  closeActionMenus()
  hideComposerMenu()
  resetStreamingPatches()
  captureTranscriptViewState()
  if (state.selectedId !== id && !leaveSessionWorkspace()) return
  state.selectedId = id
  state.selectedByBackend[state.backend] = id
  setNativeError(null)
  sessionMap.resetSelection()
  const key = sessionMapKey(state.backend, id)
  const mapLoad = sessionMap.load(state.backend, id).catch((error) => {
    console.warn('Unable to load Session Map', error)
    if (state.backend === backend && state.selectedId === id) sessionMap.setSyncState('error', error.message)
  })
  const fresh = freshThreadModel(state.backend, id)
  const cached = fresh || cachedThreadModel(state.backend, id)
  reportCodexSelectionCacheDecision(backend, id, { fresh, cached })
  state.model = cached?.model || createCodexViewModel()
  state.model.threadId = id
  prepareTranscriptViewForSelection({
    historyReady: Boolean(cached),
    historyComplete: cached?.model?.historyComplete !== false,
  })
  syncThreadListSelection()
  renderWorkspace()
  renderTranscript()
  schedulePreferencesPersist()
  const environmentLoad = activateSelectedEnvironment({ apply: Boolean(fresh) }).catch((error) => {
    reportClientError(error)
    return null
  })
  await completeSessionSelection({
    fresh: Boolean(fresh),
    cached: Boolean(cached),
    codex: isCodexBackend(backend),
    mapLoad,
    environmentLoad,
    isCurrent: () => state.backend === backend && state.selectedId === id,
    status: value => { $('#native-connection').textContent = t(value === 'cached' ? 'Restored from cache' : 'Checking for updates…') },
    resume: options => resumeThread(id, options),
    bootstrap: () => sessionMap.maybeBootstrap(key, state.model),
  })
  } finally {
    finishSelection({
      selected: state.backend === backend && state.selectedId === id,
      turnCount: state.backend === backend && state.selectedId === id && Array.isArray(state.model?.turns)
        ? state.model.turns.length
        : 0,
    })
  }
}

function markThreadLoaded(backend, id) {
  if (!addLoadedThread(state.attentionThreads, backend, id)) return
  if (state.filter === 'attention') renderThreadList()
  else renderThreadCatalogCounts()
}

function catalogThread(backend, id) {
  return (state.threadsByBackend[backend] || []).find((thread) => thread.id === id) || null
}

function nextCatalogActivityTimestamp(thread) {
  return Math.max(Date.now(), catalogActivityTimestamp(thread) + 1)
}

function refreshCatalogActivity(change) {
  if (!change?.thread || (!change.statusChanged && !change.timestampChanged)) return
  const structuralChange = (state.filter === 'attention' && change.timestampChanged)
    || (state.filter === 'active' && change.statusChanged)
  if (structuralChange) renderThreadList()
  else if (!patchThreadCatalogRow(change.backend, change.thread)) renderThreadList()
}

function setCatalogThreadActivity(backend, id, options = {}) {
  const thread = catalogThread(backend, id)
  if (!thread) return null
  const mutation = {}
  if (Object.hasOwn(options, 'status')) mutation.status = options.status
  if (options.touch) mutation.timestamp = nextCatalogActivityTimestamp(thread)
  const change = updateCatalogThreadActivity(state.threadsByBackend, backend, id, mutation)
  refreshCatalogActivity(change)
  return change
}

function rollbackCatalogThreadActivity(change) {
  const restored = restoreCatalogThreadActivity(state.threadsByBackend, change)
  refreshCatalogActivity(restored)
  return Boolean(restored)
}

function updateCodexCatalogActivity(backend, message, event, threadId) {
  if (!threadId) return null
  if (event.method === 'thread/status/changed') {
    if (!Object.hasOwn(message.params || {}, 'status')) return null
    const status = lifecycleStatusValue(message.params.status) || message.params.status
    const alreadyActive = ['active', 'running', 'inProgress'].includes(threadStatus(catalogThread(backend, threadId)))
    return setCatalogThreadActivity(backend, threadId, {
      status,
      touch: ['active', 'running', 'inProgress'].includes(lifecycleStatusValue(status)) && !alreadyActive,
    })
  }
  if (event.method === 'turn/started') {
    return setCatalogThreadActivity(backend, threadId, { status: 'active', touch: true })
  }
  if (event.method !== 'turn/completed') return null
  const terminalStatus = lifecycleStatusValue(message.params?.turn?.status || message.params?.status)
  const status = ['failed', 'systemError'].includes(terminalStatus)
    ? 'failed'
    : terminalStatus === 'interrupted' ? 'interrupted' : 'idle'
  return setCatalogThreadActivity(backend, threadId, { status, touch: true })
}

function updateOpenCodeCatalogActivity(payload, threadId) {
  if (!threadId) return null
  const thread = catalogThread('opencode', threadId)
  if (!thread) return null
  const currentStatus = threadStatus(thread)
  if (payload.type === 'session.status') {
    const status = normalizeOpenCodeStatus(payload.properties?.status)
    return setCatalogThreadActivity('opencode', threadId, {
      status,
      touch: status === 'running' && currentStatus !== 'running',
    })
  }
  if (payload.type === 'session.idle') {
    return setCatalogThreadActivity('opencode', threadId, { status: 'idle', touch: true })
  }
  if (payload.type === 'session.error') {
    return setCatalogThreadActivity('opencode', threadId, { status: 'failed', touch: true })
  }
  if (payload.type === 'session.abort') {
    return setCatalogThreadActivity('opencode', threadId, { status: 'interrupted', touch: true })
  }
  if (payload.type === 'message.updated' && payload.properties?.info?.role === 'user') {
    return setCatalogThreadActivity('opencode', threadId, {
      status: 'running',
      touch: currentStatus !== 'running',
    })
  }
  if (openCodeCompletionSignal(payload)) {
    return setCatalogThreadActivity('opencode', threadId, { touch: true })
  }
  return null
}

function updateLoadedThreadTimestamp(backend, id, options = {}) {
  if (!state.attentionThreads.has(threadCatalogKey(backend, id))) return null
  return setCatalogThreadActivity(backend, id, { ...options, touch: true })
}

function threadUpdatedAt(thread) {
  return catalogTimestamp(thread?.updatedAt || thread?.updated_at || thread?.createdAt)
}

function historyTailCapabilityKey(backend, id) {
  return `${backend}:${state.appServerGenerations[backend] ?? 'unknown'}:${id}`
}

function rememberHistoryTailCapability(backend, id, supported) {
  state.historyTailCapabilities.set(historyTailCapabilityKey(backend, id), Boolean(supported))
}

function historyTailCapability(backend, id) {
  return state.historyTailCapabilities.get(historyTailCapabilityKey(backend, id))
}

function codexTailResumeParams(threadId) {
  return {
    threadId,
    excludeTurns: true,
    initialTurnsPage: {
      limit: DEFAULT_TURN_TAIL_PAGE_SIZE,
      sortDirection: 'desc',
      itemsView: 'full',
    },
  }
}

async function prepareCodexThreadWithoutHistory(ref) {
  const capability = historyTailCapability(ref.backend, ref.id)
  if (capability === false) return dispatchBackendRpc(ref.backend, 'thread/resume', { threadId: ref.id })
  try {
    const result = await dispatchBackendRpc(ref.backend, 'thread/resume', {
      threadId: ref.id,
      excludeTurns: true,
    })
    const returnedFullHistory = Array.isArray(result?.thread?.turns) && result.thread.turns.length > 0
    rememberHistoryTailCapability(ref.backend, ref.id, !returnedFullHistory)
    return result
  } catch (error) {
    if (!isHistoryPaginationCompatibilityError(error)) throw error
    rememberHistoryTailCapability(ref.backend, ref.id, false)
    return dispatchBackendRpc(ref.backend, 'thread/resume', { threadId: ref.id })
  }
}

async function requestCodexResume(backend, id, {
  environmentRoot = '',
  environmentRevision = '',
  incremental = false,
} = {}) {
  const params = incremental ? codexTailResumeParams(id) : { threadId: id }
  const configuredResume = environmentRoot
    ? await applyEnvironmentToCodex(environmentRoot, {
      backend,
      threadId: id,
      includeThread: true,
      profileRevision: environmentRevision,
      excludeTurns: incremental,
      initialTurnsPage: incremental ? params.initialTurnsPage : null,
    })
    : null
  return configuredResume?.thread
    ? configuredResume
    : dispatchBackendRpc(backend, 'thread/resume', params)
}

async function loadCodexHistoryForSelection(backend, id, cached, options = {}) {
  return codexHistoryLoader.loadCodexHistoryForSelection(backend, id, cached, options)
}

async function loadCodexHistoryForBackground(backend, id, cached) {
  return codexHistoryLoader.loadCodexHistoryForBackground(backend, id, cached)
}

async function resumeThread(id, { environmentRoot = '', environmentRevision = '' } = {}) {
  const backend = state.backend
  const key = threadCatalogKey(backend, id)
  const historyEpoch = backend === 'opencode' ? openCodeHistoryEpoch : null
  return coordinateHistoryLoad(state.threadLoads, key, backend, historyEpoch,
    () => resumeThreadUncached(id, { environmentRoot, environmentRevision, historyEpoch }))
}

async function resumeThreadUncached(id, { environmentRoot = '', environmentRevision = '', historyEpoch = null } = {}) {
  const backend = state.backend
  const threadKey = threadCatalogKey(backend, id)
  const cached = cachedThreadModel(backend, id)
  const cachedVisible = cached?.model === state.model
  const finishHistory = studioPerformance.start('history.resume', {
    backend,
    threadKey,
    environmentConfigured: Boolean(environmentRoot),
    cachedVisible,
  })
  let historyOutcome = 'discarded'
  let historyMode = 'full'
  let tail = null
  const historyEvents = backend === 'opencode' ? beginOpenCodeHistoryEventBuffer(id) : null
  setNativeError(null)
  $('#native-connection').textContent = cachedVisible ? t('Checking for updates…') : t('Resuming session…')
  try {
    const loaded = isCodexBackend(backend)
      ? await loadCodexHistoryForSelection(backend, id, cached, {
        environmentRoot,
        environmentRevision,
      })
      : {
        result: await dispatchBackendRpc(backend, 'thread/resume', {
          threadId: id,
          ...(cachedVisible ? { historyAnchorTurnIds: cached.model.turns.map((turn) => turn?.id).filter(Boolean) } : {}),
        }),
        historyMode: cachedVisible ? 'tail' : 'full',
        tail: null,
      }
    const { result } = loaded
    historyMode = result.historyMode || loaded.historyMode
    tail = loaded.tail
    sessionDispatch.markPrepared({ backend, id })
    if (backend === 'opencode' && historyEpoch !== openCodeHistoryEpoch) return
    if (state.backend !== backend || state.selectedId !== id) return
    installSelectedHistory({
      opencode: backend === 'opencode',
      model: state.model,
      thread: result.thread,
      historyAnchorTurnId: result.historyAnchorTurnId,
      historyComplete: result.historyComplete,
      historyEvents,
      messageSnapshots: result.historyMessageSnapshots,
      statusSequence: result.historyStatusSequence,
      mergeTail: mergeOpenCodeThreadTail,
      hydrate: hydrateCodexThread,
      hydrateMetadata: hydrateOpenCodeModelMetadata,
      replay: (model, events, options) => replayOpenCodeEventsAfterHistory(model, events, id, options),
      ready: markTranscriptHistoryReady,
      mergeMetadata: mergeThreadMetadata,
      cache: model => cacheThreadModel(backend, id, model, { historyEpoch }),
    })
    $('#native-connection').textContent = t('Connected')
    renderWorkspace()
    renderTranscript()
    historyOutcome = 'loaded'
  } catch (error) {
    if (backend === 'opencode' && historyEpoch !== openCodeHistoryEpoch) return
    if (state.backend !== backend || state.selectedId !== id) return
    if (!cachedVisible) {
      abandonTranscriptHistoryRestore()
      state.model.error = error.message
      state.model.status = 'failed'
    }
    historyOutcome = cachedVisible ? 'cached-error' : 'failed'
    if (cachedVisible) $('#native-connection').textContent = t('Restored from cache')
    setNativeError(t('Unable to resume this {backend} session: {message}', { backend: currentBackend().name, message: error.message }))
    if (!cachedVisible) renderWorkspace()
  } finally {
    if (historyEvents) endOpenCodeHistoryEventBuffer(id, historyEvents)
    if (isCodexBackend(backend)) {
      const finalCache = cachedThreadModel(backend, id)
      const catalogThread = (state.threadsByBackend[backend] || []).find((candidate) => candidate.id === id)
      reportSessionLifecycle('codex-history-result', {
        backend,
        threadId: id,
        selectedId: state.backend === backend ? state.selectedId : '',
        outcome: historyOutcome,
        historyMode,
        pageCount: tail?.pageCount || 0,
        appendedTurnCount: tail?.appendedTurnCount || 0,
        modelStatus: finalCache?.model?.status || '',
        activeTurnId: finalCache?.model?.activeTurnId || '',
        catalogStatus: lifecycleStatusValue(catalogThread?.status),
        catalogUpdatedAt: threadUpdatedAt(catalogThread),
        cacheValidatedAt: Number(finalCache?.validatedAt || 0),
        socketGeneration: state.socketGeneration,
      })
    }
    finishHistory({
      outcome: historyOutcome,
      historyMode,
      pageCount: tail?.pageCount || 0,
      appendedTurnCount: tail?.appendedTurnCount || 0,
      turnCount: state.backend === backend && state.selectedId === id && Array.isArray(state.model?.turns)
        ? state.model.turns.length
        : 0,
    })
  }
}

async function refreshSelectedThread(options = {}) {
  if (!state.selectedId || isArchivedPreview()) return false
  const backend = state.backend
  const threadId = state.selectedId
  const key = threadCatalogKey(backend, threadId)
  const historyEpoch = backend === 'opencode' ? openCodeHistoryEpoch : null
  return coordinateHistoryLoad(state.threadLoads, key, backend, historyEpoch,
    () => refreshSelectedThreadUncached({ ...options, backend, threadId, historyEpoch }))
}

async function refreshSelectedThreadUncached({
  quiet = false,
  environmentRoot = '',
  environmentRevision = '',
  backend,
  threadId,
  historyEpoch,
}) {
  const threadKey = threadCatalogKey(backend, threadId)
  const finishHistory = studioPerformance.start('history.refresh', {
    backend,
    threadKey,
    quiet,
    environmentConfigured: Boolean(environmentRoot),
  })
  let historyOutcome = 'discarded'
  let historyMode = 'full'
  let tail = null
  const historyEvents = backend === 'opencode' ? beginOpenCodeHistoryEventBuffer(threadId) : null
  try {
    const loaded = isCodexBackend(backend)
      ? await loadCodexHistoryForSelection(backend, threadId, { model: state.model }, {
        environmentRoot,
        environmentRevision,
      })
      : {
        result: await dispatchBackendRpc(backend, 'thread/read', {
          threadId,
          historyAnchorTurnIds: state.model.turns.map((turn) => turn?.id).filter(Boolean),
        }),
        historyMode: 'tail',
        tail: null,
      }
    const { result } = loaded
    historyMode = result.historyMode || loaded.historyMode
    tail = loaded.tail
    if (backend === 'opencode' && historyEpoch !== openCodeHistoryEpoch) return false
    if (state.backend !== backend || state.selectedId !== threadId) return false
    if (isCodexBackend(backend)) sessionDispatch.markPrepared({ backend, id: threadId })
    installSelectedHistory({
      opencode: backend === 'opencode',
      model: state.model,
      thread: result.thread,
      historyAnchorTurnId: result.historyAnchorTurnId,
      historyComplete: result.historyComplete,
      historyEvents,
      messageSnapshots: result.historyMessageSnapshots,
      statusSequence: result.historyStatusSequence,
      mergeTail: mergeOpenCodeThreadTail,
      hydrate: hydrateCodexThread,
      hydrateMetadata: hydrateOpenCodeModelMetadata,
      replay: (model, events, options) => replayOpenCodeEventsAfterHistory(model, events, threadId, options),
      ready: markTranscriptHistoryReady,
      mergeMetadata: mergeThreadMetadata,
      cache: model => cacheThreadModel(backend, threadId, model, { historyEpoch }),
    })
    renderWorkspace()
    renderTranscript()
    if (!quiet) toast('Session refreshed')
    historyOutcome = 'loaded'
    return true
  } catch (error) {
    if (backend === 'opencode' && historyEpoch !== openCodeHistoryEpoch) return false
    if (state.backend !== backend || state.selectedId !== threadId) return false
    if (quiet) setNativeError(t('Unable to resynchronize the current {backend} session: {message}', { backend: currentBackend().name, message: error.message }))
    else showError(error)
    historyOutcome = 'failed'
    return false
  } finally {
    if (historyEvents) endOpenCodeHistoryEventBuffer(threadId, historyEvents)
    finishHistory({
      outcome: historyOutcome,
      historyMode,
      pageCount: tail?.pageCount || 0,
      appendedTurnCount: tail?.appendedTurnCount || 0,
      turnCount: state.backend === backend && state.selectedId === threadId && Array.isArray(state.model?.turns)
        ? state.model.turns.length
        : 0,
    })
  }
}

function freshThreadModel(backend, id, { reconnectValidation = false } = {}) {
  const cached = cachedThreadModel(backend, id)
  if (!cached) return null
  if (backend === 'opencode' && cached.historyEpoch !== openCodeHistoryEpoch) return null
  const thread = state.threadsByBackend[backend].find((candidate) => candidate.id === id)
  const updatedAt = thread?.updatedAt || thread?.updated_at || thread?.createdAt
  return isCatalogCacheFresh(updatedAt, cached.validatedAt, {
    coarse: reconnectValidation && isCodexBackend(backend),
  }) ? cached : null
}

function cachedThreadModel(backend, id) {
  return cachedSession(state.threadModels, backend, id)
}

function cacheThreadModel(
  backend = state.backend,
  id = state.selectedId,
  model = state.model,
  { historyEpoch = backend === 'opencode' ? openCodeHistoryEpoch : null } = {},
) {
  if (storeCachedSession(state.threadModels, backend, id, model, historyEpoch, Date.now())) {
    markThreadLoaded(backend, id)
  }
}

function invalidateThreadModel(backend, id) {
  if (!id) return
  const key = threadCatalogKey(backend, id)
  state.threadModels.delete(key)
  transcriptPresentationCache.invalidateThread(key)
  transcriptCaptureSuppressedKeys.delete(key)
  if (state.attentionThreads.delete(key)) persistPreferences()
}

function markCachedModelValidated(backend, model) {
  validateCachedModel(state.threadModels, backend, model, openCodeHistoryEpoch, Date.now())
}

function markCachedModelUnvalidated(backend, model) {
  unvalidateCachedModel(state.threadModels, backend, model)
}

function threadIdForCachedModel(backend, model) {
  if (state.backend === backend && state.model === model) return state.selectedId
  return cachedModelThreadId(state.threadModels, backend, model)
}

function codexNotificationModel(
  message,
  backend = isCodexBackend(state.backend) ? state.backend : 'codex',
) {
  return routeCodexNotification(message, {
    backend, selectedBackend: state.backend, selectedId: state.selectedId,
    selectedModel: state.model, cache: state.threadModels,
    hiddenThreads: state.hiddenCodexThreads, hiddenTurns: state.hiddenCodexTurns,
    sessionKey: sessionRefKey, turnKey: routerRuntimeKey,
  })
}

function openCodeEventThreadId(payload) {
  const properties = payload?.properties || {}
  return properties.sessionID
    || properties.sessionId
    || properties.info?.sessionID
    || properties.info?.id
    || properties.part?.sessionID
    || null
}

function hydrateOpenCodeModelMetadata(thread, model = state.model, historyComplete = true) {
  hydrateOpenCodeHistoryMetadata(thread, model, historyComplete)
}

function mergeThreadMetadata(incoming) {
  if (!incoming?.id) return
  const index = state.threads.findIndex((thread) => thread.id === incoming.id)
  const previous = index >= 0 ? state.threads[index] : null
  if (index >= 0) state.threads[index] = {
    ...mergeCatalogMetadata(backendDescriptor(state.backend).kind, state.threads[index], incoming),
    turns: undefined,
  }
  else state.threads.unshift({ ...incoming, turns: undefined })
  state.threadsByBackend[state.backend] = state.threads
  syncThreadCatalogRow(state.backend, state.threads.find((thread) => thread.id === incoming.id), previous)
}

function selectedThread() {
  const archived = sessionManagement.archive.selectedThread()
  if (archived) return archived
  return state.threads.find((thread) => thread.id === state.selectedId) || null
}

function threadForRef(ref) {
  return state.threadsByBackend[ref?.backend]?.find((thread) => thread.id === ref?.id) || null
}

function isArchivedPreview() {
  return sessionManagement.archive.isPreview()
}

function selectedStateKey(id = state.selectedId, backend = state.backend) {
  return id ? `${backend}:${id}` : ''
}

function syncComposerDraft() {
  const input = $('#composer-input')
  if (!input) return
  const value = composerDrafts.switchTo(selectedStateKey(), input.value)
  if (input.value === value) return
  input.value = value
  input.setSelectionRange(value.length, value.length)
}

function setComposerDraftValue(key, value) {
  const normalized = String(value || '')
  composerDrafts.update(key, normalized)
  if (key !== selectedStateKey()) return
  const input = $('#composer-input')
  input.value = normalized
}

function setCurrentComposerValue(value) {
  setComposerDraftValue(selectedStateKey(), value)
}

function discardComposerSessionState(backend, threadId) {
  const key = selectedStateKey(threadId, backend)
  if (!key) return
  composerDrafts.discard(key)
  delete state.pendingSkills[key]
  delete state.pendingFiles[key]
  delete state.pendingImages[key]
}

function captureOpeningMessage() {
  const key = selectedStateKey()
  if (!key || state.openingMessages[key]?.text) return
  let text = ''
  for (const turn of state.model.turns) {
    text = questionForTurn(turn).trim()
    if (text) break
  }
  if (!text) return
  const normalized = truncateUtf8(text, 16 * 1024)
  state.openingMessages[key] = {
    ...(state.openingMessages[key] || {}),
    text: normalized,
    source: 'history',
    capturedAt: new Date().toISOString(),
    truncated: normalized !== text,
  }
  persistOpeningMessageState(key)
}

function openThreadInfo() {
  const thread = selectedThread()
  if (!thread) return
  captureOpeningMessage()
  const opening = state.openingMessages[selectedStateKey()]
  const status = state.model.status === 'disconnected' ? threadStatus(thread) : state.model.status
  const source = threadSourceLabel(thread.source)
  const routable = !isRouterThread()
  $('#thread-info-content').innerHTML = `
    <section class="opening-message-card">
      <header><strong>${t('Opening question')}</strong><span>${opening ? `${opening.source === 'history' ? t('Extracted from history') : escapeHtml(opening.source)}${opening.truncated ? ` · ${t('Truncated')}` : ''}` : t('Not identified')}</span></header>
      <textarea id="session-opening-question" rows="4" maxlength="16384" placeholder="${t('No opening user message was found in the structured history.')}">${escapeHtml(opening?.text || '')}</textarea>
    </section>
    ${routable ? `<section class="session-responsibility-card">
      <header><div><strong>${t('Session responsibility')}</strong><small>${t('Used by Thread Router to decide which requests should be dispatched to this session.')}</small></div><button id="generate-session-responsibility" class="subtle-button compact" type="button">${t('Generate with AI')}</button></header>
      <textarea id="session-responsibility" rows="4" placeholder="${t('Summarize the domain this session owns and the requests it should handle')}">${escapeHtml(opening?.responsibility || '')}</textarea>
    </section>` : ''}
    <div class="detail-row"><span>Status</span><strong>${escapeHtml(statusLabel(status))}</strong></div>
    <div class="detail-row"><span>Backend</span><strong>${escapeHtml(currentBackend().name)}</strong></div>
    <div class="detail-row"><span>Session ID</span><strong data-no-i18n>${escapeHtml(thread.id)}</strong></div>
    ${thread.sessionId && thread.sessionId !== thread.id ? `<div class="detail-row"><span>Session tree</span><strong data-no-i18n>${escapeHtml(thread.sessionId)}</strong></div>` : ''}
    <div class="detail-row"><span>Project directory</span><strong data-no-i18n>${escapeHtml(thread.cwd || t('Not recorded'))}</strong></div>
    ${source ? `<div class="detail-row"><span>Source</span><strong data-no-i18n>${escapeHtml(source)}</strong></div>` : ''}
    ${thread.cliVersion ? `<div class="detail-row"><span>CLI Version</span><strong data-no-i18n>${escapeHtml(thread.cliVersion)}</strong></div>` : ''}
    ${thread.forkedFromId ? `<div class="detail-row"><span>Fork source</span><strong data-no-i18n>${escapeHtml(thread.forkedFromId)}</strong></div>` : ''}
    ${thread.parentThreadId ? `<div class="detail-row"><span>Parent session</span><strong data-no-i18n>${escapeHtml(thread.parentThreadId)}</strong></div>` : ''}`
  $('#copy-opening-message').disabled = !opening?.text
  $('#generate-session-responsibility')?.addEventListener('click', () => generateSessionResponsibility().catch(showError))
  $('#thread-info-dialog').showModal()
}

async function saveThreadInfo() {
  const key = selectedStateKey()
  if (!key) return
  const hadExisting = Object.hasOwn(state.openingMessages, key)
  const existing = state.openingMessages[key] || {}
  const text = truncateUtf8($('#session-opening-question')?.value.trim() || '', 16 * 1024)
  const responsibility = truncateCharacters(($('#session-responsibility')?.value || '').trim(), 4096)
  if (!text && !responsibility) delete state.openingMessages[key]
  else {
    state.openingMessages[key] = {
      ...existing,
      text,
      responsibility,
      source: text === existing.text ? existing.source || 'history' : 'manual',
      capturedAt: new Date().toISOString(),
      truncated: false,
    }
  }
  try {
    await persistOpeningMessageState(key)
    $('#thread-info-dialog').close()
    toast(t('Session information saved'))
  } catch (error) {
    if (hadExisting) state.openingMessages[key] = existing
    else delete state.openingMessages[key]
    showError(error)
  }
}

function applyTurnAcknowledgement(model, turn, threadId = '') {
  return submissionController.applyTurnAcknowledgement(model, turn, threadId)
}

async function generateSessionResponsibility() {
  if (!state.selectedId || isRouterThread()) return
  if (state.model.activeTurnId) throw new Error(t('Wait for the current turn to finish before generating a session responsibility.'))
  const ref = { backend: state.backend, id: state.selectedId }
  const key = sessionRefKey(ref.backend, ref.id)
  const button = $('#generate-session-responsibility')
  button.disabled = true
  button.textContent = t('Generating…')
  const opening = $('#session-opening-question')?.value.trim() || state.openingMessages[key]?.text || ''
  const prompt = `Generate a concise session summary that can be used as this session's responsibility for routing future user queries. Describe the domain this session owns and the kinds of requests it should receive. Use the session conversation as primary context.${opening ? ` The opening question is:\n${opening}` : ''}\nReturn only the responsibility summary, with no preface or formatting.`
  try {
    const result = await sessionDispatch.startTurn(ref, [{ type: 'text', text: prompt }], {
      turnOptions: configuredTurnOptions(),
      timeoutMs: 60_000,
    })
    if (!result?.turn?.id) throw new Error(t('Unable to start session responsibility generation.'))
    applyTurnAcknowledgement(state.model, result.turn, ref.id)
    cacheThreadModel(ref.backend, ref.id, state.model)
    renderTranscript()
    renderComposerState()
    const turn = await waitForSessionTurn(ref, result.turn.id, 300_000)
    const responsibility = truncateCharacters(finalAgentText(turn).trim(), 4096)
    if (!responsibility) throw new Error(t('AI did not return a session responsibility summary.'))
    if ($('#thread-info-dialog').open && selectedStateKey() === key) $('#session-responsibility').value = responsibility
    toast(t('Session responsibility generated. Review it before saving.'))
  } finally {
    if (button.isConnected) {
      button.disabled = false
      button.textContent = t('Generate with AI')
    }
  }
}

async function waitForSessionTurn(ref, turnId, timeoutMs) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const model = await ensureSessionModel(ref)
    const turn = model.turns?.find((candidate) => String(candidate.id) === String(turnId))
    if (turn && turn.status !== 'inProgress' && model.status !== 'running') {
      if (turn.status === 'failed') throw new Error(turn.error?.message || t('Session responsibility generation failed.'))
      if (state.backend === ref.backend && state.selectedId === ref.id) {
        renderTranscript()
        renderComposerState()
      }
      return turn
    }
    await new Promise((resolve) => setTimeout(resolve, 900))
  }
  throw new Error(t('Session responsibility generation timed out.'))
}

async function copyOpeningMessage() {
  const text = state.openingMessages[selectedStateKey()]?.text
  if (!text) return
  await navigator.clipboard.writeText(text)
  toast('Opening question copied')
}

function renderWorkspace() {
  syncComposerDraft()
  const thread = selectedThread()
  const hasThread = Boolean(thread)
  const archived = hasThread && isArchivedPreview()
  $('#thread-heading').classList.toggle('hidden', !hasThread)
  $('#thread-actions').classList.toggle('hidden', !hasThread)
  $('#empty-workspace').classList.toggle('hidden', hasThread)
  $('#native-workspace').classList.toggle('hidden', !hasThread)
  $('#composer-form').classList.toggle('hidden', archived)
  $('#archived-composer').classList.toggle('hidden', !archived)
  $('#thread-more-button').classList.toggle('hidden', !hasThread)
  for (const id of ['rename-thread', 'fork-thread', 'pin-thread', 'archive-thread', 'refresh-thread', 'project-environment-action', 'session-map-action', 'router-settings-action']) {
    $(`#${id}`).classList.toggle('hidden', archived)
  }
  $('.thread-action-menu .menu-separator')?.classList.toggle('hidden', archived)
  workspaceTools.sync(thread)
  sessionResources.syncSelection()
  if (!thread) {
    sessionManagement.search.close()
    sessionMap.render()
    return
  }
  $('#thread-title').textContent = threadTitle(thread)
  $('#thread-path').textContent = thread.cwd || thread.id
  $('#archive-thread').disabled = archived || state.backend === 'opencode'
  $('#archive-thread').title = t(state.backend === 'opencode' ? 'The OpenCode backend does not support archiving yet' : 'Archive session')
  syncPinThreadAction()
  $('#router-settings-action').classList.toggle('hidden', archived || !isRouterThread())
  if (!archived) {
    renderProjectEnvironmentEntry()
    renderComposerState()
  }
  reviewNotes.renderAnnotations()
  captureOpeningMessage()
  reviewNotes.renderSessionFavoriteCount()
  if (!restoreSessionWorkspace()) sessionMap.render()
}

const sessionWorkspaceMemory = new SessionWorkspaceMemory()
let restoredWorkspaceKey = ''
function leaveSessionWorkspace() {
  const key = selectedStateKey()
  const visible = id => !$(`#${id}`).classList.contains('hidden')
  const file = state.artifact
  const workspace = workspaceTools.snapshot()
  const tool = file && visible('artifact-rail') ? 'document' : workspace ? 'workspace'
    : visible('annotation-rail') ? 'comments' : visible('favorites-rail') ? 'favorites'
      : sessionResources.isOpen() ? 'resources' : state.embeddedBrowserVisible ? 'browser'
        : visible('session-map-rail') ? 'map' : null
  const snapshot = { tool, workspace, favoriteScope: state.favoriteScope }
  if (tool === 'document') snapshot.document = {
    file: { root: file.root, path: file.path, sourceSessionKey: file.sourceSessionKey, epubCfi: file.readingState?.cfi || '' },
    returnTool: file.returnTool,
    viewState: { view: state.artifactView, page: file.page, scrollTop: $('#artifact-content').scrollTop, scrollLeft: $('#artifact-content').scrollLeft,
      outlineOpen: state.artifactOutlineOpen, outlineFilter: state.artifactOutlineFilter, outlineCollapsed: [...state.artifactOutlineCollapsed],
      search: state.artifactSearch, searchOpen: state.artifactSearchOpen },
  }
  if (tool === 'document' && file.loading) {
    const previous = sessionWorkspaceMemory.get(key)?.document
    if (previous?.file.path === file.path && previous.file.root === file.root) snapshot.document = previous
  }
  if (file && documentWorkspace.close({ restoreMap: false, restoreWorkspace: false }) === false) return false
  sessionWorkspaceMemory.remember(key, snapshot)
  sessionWorkspaceMemory.invalidate()
  restoredWorkspaceKey = ''
  sessionResources.close()
  activateRightWorkspace(null)
  return true
}

function restoreSessionWorkspace() {
  const key = selectedStateKey()
  if (!key || restoredWorkspaceKey === key) return false
  restoredWorkspaceKey = key
  const snapshot = sessionWorkspaceMemory.get(key)
  if (!snapshot) return false
  const valid = sessionWorkspaceMemory.begin()
  const current = () => valid() && key === selectedStateKey()
  const restore = async () => {
    if (snapshot.tool === 'document') {
      await documentWorkspace.open(snapshot.document.file, { returnTool: snapshot.document.returnTool, viewState: snapshot.document.viewState })
    } else if (snapshot.tool === 'workspace') {
      const { source, tool, scroll } = snapshot.workspace
      if (source) {
        const thread = threadForRef(source)
        if (!thread) return
        await workspaceTools.openForSession(thread, source.backend, tool)
      } else await workspaceTools.open(tool)
      if (current() && workspaceTools.snapshot()?.tool === tool) for (const position of scroll) {
        const node = $(`#${position.id}`)
        if (node) { node.scrollTop = position.top; node.scrollLeft = position.left }
      }
    } else if (snapshot.tool === 'comments') reviewNotes.openAnnotations()
    else if (snapshot.tool === 'favorites') await reviewNotes.openFavorites(snapshot.favoriteScope)
    else if (snapshot.tool === 'resources') sessionResources.open()
    else if (snapshot.tool === 'browser') await openGlobalBrowser()
    else if (snapshot.tool === 'map') sessionMap.render()
  }
  restore().catch(error => { if (current()) showError(error) })
  return true
}

function resetStreamingPatches() {
  if (transcriptFrame != null) cancelAnimationFrame(transcriptFrame)
  transcriptFrame = null
  dirtyStreamItems.clear()
}


function presentationThreadKey(backend = state.backend, id = state.selectedId) {
  return threadCatalogKey(backend, id || 'none')
}

function currentPresentationEntry() {
  return transcriptPresentationCache.get(presentationThreadKey(), state.model)
}

function transcriptReadingAnchor(container = $('#transcript')) {
  const containerRect = container.getBoundingClientRect()
  const turns = [...container.querySelectorAll('.turn[data-turn-id]')]
  const turn = turns.find((candidate) => candidate.getBoundingClientRect().bottom > containerRect.top + 1)
    || turns.at(-1)
  if (!turn) return { turnId: '', offset: 0 }
  return {
    turnId: turn.dataset.turnId || '',
    offset: turn.getBoundingClientRect().top - containerRect.top,
  }
}

function captureTranscriptRenderAnchor(container = $('#transcript')) {
  if (pendingTranscriptViewRestore?.key === presentationThreadKey() || transcriptScrollFollower.following) return null
  const containerRect = container.getBoundingClientRect()
  const targets = [...container.querySelectorAll('[data-turn-id][data-item-id], .work-activity[data-turn-id][data-activity-id]')]
  const item = targets.find((candidate) => candidate.getBoundingClientRect().bottom > containerRect.top + 1)
  const turn = item?.closest('.turn[data-turn-id]')
    || [...container.querySelectorAll('.turn[data-turn-id]')]
      .find((candidate) => candidate.getBoundingClientRect().bottom > containerRect.top + 1)
  const target = item || turn
  if (!target) return null
  return {
    turnId: turn?.dataset.turnId || target.dataset.turnId || '',
    itemId: item?.dataset.itemId || '',
    activityId: item?.dataset.activityId || '',
    offset: target.getBoundingClientRect().top - containerRect.top,
  }
}

function restoreTranscriptRenderAnchor(anchor, container = $('#transcript')) {
  if (!anchor) return false
  const turns = [...container.querySelectorAll('.turn[data-turn-id]')]
  const turn = turns.find((candidate) => candidate.dataset.turnId === anchor.turnId)
  const item = anchor.itemId && turn
    ? [...turn.querySelectorAll('[data-item-id]')].find((candidate) => candidate.dataset.itemId === anchor.itemId)
    : null
  const activity = anchor.activityId && turn
    ? [...turn.querySelectorAll('.work-activity[data-activity-id]')]
      .find((candidate) => candidate.dataset.activityId === anchor.activityId)
    : null
  const target = item || activity || turn
  if (!target) return false
  const containerRect = container.getBoundingClientRect()
  container.scrollTop += target.getBoundingClientRect().top - containerRect.top - Number(anchor.offset || 0)
  return true
}

function clearTranscriptLiveLayoutAnchor(key = '') {
  if (!key || transcriptLiveLayoutAnchor?.key === key) transcriptLiveLayoutAnchor = null
}

function rememberTranscriptLiveLayoutAnchor(container = $('#transcript')) {
  const key = presentationThreadKey()
  if (transcriptScrollFollower.following || pendingTranscriptViewRestore?.key === key) {
    clearTranscriptLiveLayoutAnchor(key)
    return null
  }
  const anchor = captureTranscriptRenderAnchor(container)
  transcriptLiveLayoutAnchor = anchor ? { key, anchor } : null
  return anchor
}

function restoreTranscriptLiveLayoutAnchor(container = $('#transcript')) {
  const key = presentationThreadKey()
  if (transcriptLiveLayoutAnchor?.key !== key) return false
  if (!restoreTranscriptRenderAnchor(transcriptLiveLayoutAnchor.anchor, container)) {
    clearTranscriptLiveLayoutAnchor(key)
    return false
  }
  scheduleTranscriptViewCapture()
  return true
}

function preserveTranscriptLayout(mutate) {
  const key = presentationThreadKey()
  const container = $('#transcript')
  const hasPendingRestore = pendingTranscriptViewRestore?.key === key
  const wasFollowing = transcriptScrollFollower.following
  const anchor = hasPendingRestore || wasFollowing ? null : captureTranscriptRenderAnchor(container)
  mutate()
  if (hasPendingRestore) return
  requestAnimationFrame(() => {
    if (key !== presentationThreadKey()) return
    if (wasFollowing) followTranscriptOutput()
    else if (restoreTranscriptRenderAnchor(anchor, container)) scheduleTranscriptViewCapture()
  })
}

function transcriptReadingTurnId(container = $('#transcript')) {
  const containerRect = container.getBoundingClientRect()
  const marker = containerRect.top + Math.min(container.clientHeight * 0.28, 160)
  const positions = [...container.querySelectorAll('.turn[data-turn-id]')]
    .map((turn) => ({ id: turn.dataset.turnId, top: turn.getBoundingClientRect().top }))
  return activeTurnAtMarker(positions, marker, distanceFromBottom(container) < 8) || ''
}

function captureTranscriptViewState() {
  cancelScheduledTranscriptViewCapture()
  if (!state.selectedId) return
  const key = presentationThreadKey()
  const container = $('#transcript')
  if (transcriptCaptureSuppressedKeys.has(key)) return
  // A return-position restore owns the viewport until it is explicitly applied,
  // rejected, or cancelled by a user navigation action. DOM clamping while an
  // uncached history is loading must never consume the transaction.
  if (pendingTranscriptViewRestore?.key === key) return
  const orderedTurnIds = transcriptPresentationCache.peek(key)?.orderedIds
    || (state.model.turns || []).map((turn) => String(turn?.id || '')).filter(Boolean)
  const anchor = transcriptReadingAnchor(container)
  const readingTurnId = transcriptReadingTurnId(container)
  const bottomDistance = distanceFromBottom(container)
  transcriptPresentationCache.setScrollState(key, {
    scrollTop: container.scrollTop,
    anchorTurnId: anchor.turnId,
    anchorOffset: anchor.offset,
    followOnReturn: shouldFollowLatestOnReturn({
      orderedTurnIds,
      readingTurnId,
      bottomDistance,
    }),
  })
  rememberTranscriptLiveLayoutAnchor(container)
}

function scheduleTranscriptViewCapture() {
  const key = presentationThreadKey()
  if (transcriptCaptureFrame != null && transcriptCaptureFrameKey === key) return
  if (transcriptCaptureFrame != null) cancelAnimationFrame(transcriptCaptureFrame)
  transcriptCaptureFrameKey = key
  transcriptCaptureFrame = requestAnimationFrame(() => {
    transcriptCaptureFrame = null
    const scheduledKey = transcriptCaptureFrameKey
    transcriptCaptureFrameKey = ''
    if (scheduledKey === presentationThreadKey()) captureTranscriptViewState()
  })
}

function cancelScheduledTranscriptViewCapture() {
  if (transcriptCaptureFrame != null) cancelAnimationFrame(transcriptCaptureFrame)
  transcriptCaptureFrame = null
  transcriptCaptureFrameKey = ''
}

function prepareTranscriptViewForSelection({ historyReady = true, historyComplete = true } = {}) {
  transcriptLiveLayoutAnchor = null
  transcriptUserScrollIntentUntil = 0
  transcriptPointerScrollActive = false
  if (!state.selectedId) {
    pendingTranscriptViewRestore = null
    transcriptScrollFollower.reset()
    return
  }
  const key = presentationThreadKey()
  if (historyReady) transcriptCaptureSuppressedKeys.delete(key)
  const saved = transcriptPresentationCache.scrollState(key)
  if (saved && !saved.followOnReturn) {
    pendingTranscriptViewRestore = {
      key,
      ...saved,
      historyReady: Boolean(historyReady),
      historyComplete: historyComplete !== false,
    }
    transcriptScrollFollower.pause()
  } else {
    pendingTranscriptViewRestore = null
    transcriptScrollFollower.reset()
  }
}

function markTranscriptHistoryReady({ complete = true } = {}) {
  const key = presentationThreadKey()
  const wasSuppressed = transcriptCaptureSuppressedKeys.delete(key)
  if (wasSuppressed && pendingTranscriptViewRestore?.key !== key) {
    const saved = transcriptPresentationCache.scrollState(key)
    if (saved && !saved.followOnReturn) {
      pendingTranscriptViewRestore = {
        key,
        ...saved,
        historyReady: true,
        historyComplete: complete !== false,
      }
      transcriptScrollFollower.pause()
    }
  }
  if (pendingTranscriptViewRestore?.key === key) {
    pendingTranscriptViewRestore.historyReady = true
    pendingTranscriptViewRestore.historyComplete = complete !== false
  }
}

function abandonTranscriptHistoryRestore() {
  if (pendingTranscriptViewRestore?.key !== presentationThreadKey()) return
  transcriptCaptureSuppressedKeys.add(presentationThreadKey())
  pendingTranscriptViewRestore = null
  clearTranscriptLiveLayoutAnchor(presentationThreadKey())
  transcriptScrollFollower.reset()
}

function transcriptRestoreContext(entry) {
  const key = presentationThreadKey()
  const pending = pendingTranscriptViewRestore?.key === key ? pendingTranscriptViewRestore : null
  if (!pending) return { key, pending: null, saved: null, plan: { type: 'none' } }
  const resolvedEntry = entry || currentPresentationEntry()
  return {
    key,
    pending,
    saved: pending,
    plan: transcriptRestorePlan({
      saved: pending,
      historyReady: pending.historyReady,
      historyComplete: pending.historyComplete,
      orderedTurnIds: resolvedEntry.orderedIds,
    }),
  }
}

function prepareTranscriptEntryForRestore() {
  const key = presentationThreadKey()
  const cachedEntry = transcriptPresentationCache.peekCurrent(key, state.model)
  if (transcriptScrollFollower.following) return cachedEntry || currentPresentationEntry()
  const restore = transcriptRestoreContext(cachedEntry || {
    orderedIds: (state.model.turns || []).map((turn) => String(turn?.id || '')).filter(Boolean),
  })
  if (restore.plan.type === 'anchor') {
    return transcriptPresentationCache.restoreTurn(restore.key, state.model, restore.plan.anchorTurnId)
  }
  if (restore.plan.type === 'stale') {
    if (restore.pending) pendingTranscriptViewRestore = null
    transcriptPresentationCache.setScrollState(restore.key, null)
    clearTranscriptLiveLayoutAnchor(restore.key)
    transcriptScrollFollower.reset()
  }
  if (restore.plan.type === 'unavailable') {
    transcriptCaptureSuppressedKeys.add(restore.key)
    if (restore.pending) pendingTranscriptViewRestore = null
    clearTranscriptLiveLayoutAnchor(restore.key)
    transcriptScrollFollower.reset()
  }
  return cachedEntry || currentPresentationEntry()
}

function restoreTranscriptView(container = $('#transcript'), entry = null) {
  if (!state.selectedId || pendingTranscriptViewRestore?.key !== presentationThreadKey()) return false
  const restore = transcriptRestoreContext(entry)
  if (restore.plan.type === 'none' || restore.plan.type === 'defer') return false
  if (restore.pending) pendingTranscriptViewRestore = null
  if (restore.plan.type === 'stale') {
    transcriptPresentationCache.setScrollState(restore.key, null)
    clearTranscriptLiveLayoutAnchor(restore.key)
    transcriptScrollFollower.reset()
    return false
  }
  if (restore.plan.type === 'unavailable') {
    transcriptCaptureSuppressedKeys.add(restore.key)
    clearTranscriptLiveLayoutAnchor(restore.key)
    transcriptScrollFollower.reset()
    return false
  }
  const anchor = restore.plan.type === 'anchor'
    ? [...container.querySelectorAll('.turn[data-turn-id]')]
      .find((turn) => turn.dataset.turnId === restore.plan.anchorTurnId)
    : null
  if (anchor) {
    const containerRect = container.getBoundingClientRect()
    container.scrollTop += anchor.getBoundingClientRect().top - containerRect.top - Number(restore.saved.anchorOffset || 0)
    rememberTranscriptLiveLayoutAnchor(container)
    return true
  }
  if (restore.plan.type === 'scrollTop') {
    container.scrollTop = restore.plan.scrollTop
    rememberTranscriptLiveLayoutAnchor(container)
    return true
  }
  transcriptPresentationCache.setScrollState(restore.key, null)
  clearTranscriptLiveLayoutAnchor(restore.key)
  transcriptScrollFollower.reset()
  return false
}

function renderTranscript({ preserveScroll = false, previousHeight = 0, previousTop = 0 } = {}) {
  if (!state.selectedId) return
  threadRouter.syncDirectTurns()
  const threadKey = presentationThreadKey()
  const presentationCacheHit = Boolean(transcriptPresentationCache.peekCurrent(threadKey, state.model))
  const finishRender = studioPerformance.start('transcript.render', {
    backend: state.backend,
    threadKey,
    turnCount: Array.isArray(state.model?.turns) ? state.model.turns.length : 0,
    presentationCacheHit,
  })
  resetStreamingPatches()
  const container = $('#transcript')
  // A same-session render can replace changed Turn nodes. Preserve the live
  // viewport with a one-render anchor while paused; a follower must remain at
  // the latest output instead of restoring the node that preceded a new turn.
  const renderAnchor = preserveScroll || transcriptScrollFollower.following
    ? null
    : captureTranscriptRenderAnchor(container)
  const openActivities = new Map()
  for (const activity of container.querySelectorAll('.work-activity[open][data-turn-id][data-activity-id]')) {
    const ids = openActivities.get(activity.dataset.turnId) || []
    ids.push(activity.dataset.activityId)
    openActivities.set(activity.dataset.turnId, ids)
  }
  const finishPresentation = studioPerformance.start('transcript.presentation', {
    backend: state.backend,
    threadKey,
    presentationCacheHit,
  })
  const entry = prepareTranscriptEntryForRestore()
  finishPresentation({
    visibleTurns: Math.max(0, entry.visibleEnd - entry.visibleStart),
  })
  const finishDom = studioPerformance.start('transcript.dom', {
    backend: state.backend,
    threadKey,
  })
  const visibleIds = entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd)
  const older = entry.visibleStart > 0
    ? `<button class="load-earlier-turns" type="button" data-load-earlier>${t('{count} earlier turns', { count: entry.visibleStart })}</button>`
    : ''
  const hasLaterTurns = entry.visibleEnd < entry.orderedIds.length
  const later = hasLaterTurns
    ? `<button class="load-earlier-turns" type="button" data-jump-latest>${t('Jump to latest')}</button>`
    : ''
  const chunks = visibleIds.map((id, offset) => {
    const index = entry.visibleStart + offset
    const turn = entry.sourceTurns[index]
    if (String(turn?.id || '') !== id || !turn) return { id, html: '' }
    const html = isRouterThread() ? threadRouter.renderTurn(turn, index)
      : renderTurn(entry.turns.get(id)?.presentation, index, { openActivityIds: openActivities.get(id) || [] })
    return { id, html }
  })
  chunks.unshift({ id: '__older', html: older })
  chunks.push({ id: '__footer', html: later + renderApprovals() })
  transcriptItemNodes.clear()
  const domResult = transcriptDom.render(container, threadKey, chunks)
  finishDom({ visibleTurns: visibleIds.length, htmlLength: chunks.reduce((size, chunk) => size + chunk.html.length, 0), ...domResult })
  const finishPostprocess = studioPerformance.start('transcript.postprocess', {
    backend: state.backend,
    threadKey,
  })
  observeTranscriptContent()
  bindApprovalButtons()
  bindActivityDetails()
  threadRouter.renderAttention()
  reviewNotes.renderCommentMarkers()
  scheduleTurnNavigatorRender()
  if (preserveScroll) {
    container.scrollTop = previousTop + Math.max(0, container.scrollHeight - previousHeight)
    captureTranscriptViewState()
  } else if (!restoreTranscriptView(container, entry)) {
    if (restoreTranscriptRenderAnchor(renderAnchor, container)) scheduleTranscriptViewCapture()
    else followTranscriptOutput()
  }
  captureOpeningMessage()
  sessionResources.sync()
  finishPostprocess({ visibleTurns: visibleIds.length })
  finishRender({ visibleTurns: visibleIds.length })
}

function markTranscriptUserScrollIntent(duration = 500) {
  transcriptUserScrollIntentUntil = Math.max(
    transcriptUserScrollIntentUntil,
    Date.now() + Math.max(0, Number(duration) || 0),
  )
}

function transcriptUserScrollIntentActive() {
  return transcriptPointerScrollActive || Date.now() <= transcriptUserScrollIntentUntil
}

function beginTranscriptProgrammaticNavigation() {
  const key = presentationThreadKey()
  if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
  transcriptCaptureSuppressedKeys.delete(key)
  transcriptScrollFollower.pause()
  transcriptPresentationCache.pinCurrent(key)
  rememberTranscriptLiveLayoutAnchor()
  markTranscriptUserScrollIntent(1_200)
}

function beginTranscriptFollowingLatest(model = state.model) {
  const key = presentationThreadKey()
  cancelScheduledTranscriptViewCapture()
  if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
  transcriptCaptureSuppressedKeys.delete(key)
  transcriptPresentationCache.setScrollState(key, null)
  clearTranscriptLiveLayoutAnchor(key)
  transcriptScrollFollower.reset()
  transcriptPresentationCache.followLatest(key, model)
}

function handleTranscriptScroll() {
  const transcript = $('#transcript')
  const key = presentationThreadKey()
  const action = transcriptScrollEventAction({
    hasPendingRestore: pendingTranscriptViewRestore?.key === key,
    userInitiated: transcriptUserScrollIntentActive(),
    following: transcriptScrollFollower.following,
  })
  if (action === 'ignore') {
    scheduleTurnNavigatorSync()
    return
  }
  if (action === 'follow') {
    followTranscriptOutput()
    return
  }
  if (action === 'preserve') {
    scheduleTurnNavigatorSync()
    return
  }
  markTranscriptUserScrollIntent(240)
  const entry = transcriptPresentationCache.peek(key)
  const hasLaterTurns = Boolean(entry && entry.visibleEnd < entry.orderedIds.length)
  if (hasLaterTurns) {
    transcriptScrollFollower.pause()
    transcriptPresentationCache.pinCurrent(key)
  }
  else if (transcriptScrollFollower.handleScroll(transcript)) {
    transcriptPresentationCache.followLatest(key, state.model)
  } else {
    transcriptPresentationCache.pinCurrent(key)
  }
  scheduleTranscriptViewCapture()
  scheduleTurnNavigatorSync()
}

function handleTranscriptUserTakeover(event) {
  const key = presentationThreadKey()
  const transcript = $('#transcript')
  if (!transcript.querySelector('.turn[data-turn-id]')) return
  if (event?.type === 'pointerdown') transcriptPointerScrollActive = true
  markTranscriptUserScrollIntent(event?.type === 'touchstart' ? 900 : 500)
  if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
  transcriptCaptureSuppressedKeys.delete(key)
  const entry = transcriptPresentationCache.peek(key)
  const hasLaterTurns = Boolean(entry && entry.visibleEnd < entry.orderedIds.length)
  if (shouldPinTranscriptOnTakeover({ metrics: transcript, hasLaterTurns })) {
    transcriptScrollFollower.pause()
    transcriptPresentationCache.pinCurrent(key)
    rememberTranscriptLiveLayoutAnchor(transcript)
  } else {
    transcriptScrollFollower.reset()
    if (entry?.windowMode !== 'latest') transcriptPresentationCache.followLatest(key, state.model)
  }
}

function handleTranscriptKeyboardTakeover(event) {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || isTypingTarget(event.target)) return
  if (!['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) return
  handleTranscriptUserTakeover(event)
}

function finishTranscriptUserTakeover(event) {
  if (event?.type === 'pointerup' || event?.type === 'pointercancel') {
    if (!transcriptPointerScrollActive) return
    transcriptPointerScrollActive = false
  }
  markTranscriptUserScrollIntent(event?.type?.startsWith('touch') ? 700 : 180)
}

function followTranscriptOutput() {
  if (!transcriptScrollFollower.following) return
  const transcript = $('#transcript')
  clearTranscriptLiveLayoutAnchor(presentationThreadKey())
  transcript.scrollTop = transcript.scrollHeight
  scheduleTurnNavigatorSync()
}

function observeTranscriptContent() {
  transcriptContentObserver.observe($('#transcript'))
}

function handleTranscriptContentResize() {
  // Session-return positions are one-shot. Once a selection restore has been
  // consumed, ordinary layout changes (for example closing the Comment rail)
  // preserve the live DOM anchor instead of replaying an old session snapshot.
  const action = transcriptResizeAction({
    hasPendingRestore: pendingTranscriptViewRestore?.key === presentationThreadKey(),
    following: transcriptScrollFollower.following,
  })
  if (action === 'restore') restoreTranscriptView()
  else if (action === 'follow') followTranscriptOutput()
  else if (transcriptUserScrollIntentActive()) scheduleTranscriptViewCapture()
  else restoreTranscriptLiveLayoutAnchor()
  scheduleTurnNavigatorSync()
}

function cancelScheduledTurnNavigatorRender() {
  if (turnNavigatorRenderFrame != null) cancelAnimationFrame(turnNavigatorRenderFrame)
  if (turnNavigatorRenderIdle != null && typeof globalThis.cancelIdleCallback === 'function') {
    globalThis.cancelIdleCallback(turnNavigatorRenderIdle)
  }
  if (turnNavigatorRenderTimer != null) clearTimeout(turnNavigatorRenderTimer)
  turnNavigatorRenderFrame = null
  turnNavigatorRenderIdle = null
  turnNavigatorRenderTimer = null
  turnNavigatorRenderKey = ''
}

function scheduleTurnNavigatorRender() {
  const key = presentationThreadKey()
  const alreadyScheduled = turnNavigatorRenderKey === key
    && (turnNavigatorRenderFrame != null || turnNavigatorRenderIdle != null || turnNavigatorRenderTimer != null)
  if (alreadyScheduled) return
  cancelScheduledTurnNavigatorRender()
  turnNavigatorRenderKey = key
  const generation = ++turnNavigatorRenderGeneration
  if (turnNavigatorRenderedKey !== key) $('#turn-navigator').classList.add('hidden')

  const run = () => {
    turnNavigatorRenderIdle = null
    turnNavigatorRenderTimer = null
    turnNavigatorRenderKey = ''
    if (generation !== turnNavigatorRenderGeneration || key !== presentationThreadKey()) return
    renderTurnNavigator()
  }
  const afterPaint = () => {
    turnNavigatorRenderFrame = null
    if (generation !== turnNavigatorRenderGeneration || key !== presentationThreadKey()) return
    if (typeof globalThis.requestIdleCallback === 'function') {
      turnNavigatorRenderIdle = globalThis.requestIdleCallback(run, { timeout: 750 })
    } else {
      turnNavigatorRenderTimer = setTimeout(run, 0)
    }
  }
  turnNavigatorRenderFrame = requestAnimationFrame(afterPaint)
}

function renderTurnNavigator() {
  const unreadTurns = threadRouter.unreadTurnIds()
  const threadKey = presentationThreadKey()
  const finishRender = studioPerformance.start('turnNavigator.render', {
    backend: state.backend,
    threadKey,
    turnCount: Array.isArray(state.model?.turns) ? state.model.turns.length : 0,
  })
  const navigator = $('#turn-navigator')
  const list = $('#turn-navigator-list')
  const turns = navigableTurns(state.model.turns)
  const items = turns.map((turn, index) => {
    const preview = turnPromptPreview(turn)
    const fallback = t('User input {index}', { index: index + 1 })
    return {
      id: String(turn.id || ''),
      label: preview ? `${fallback}: ${preview}` : fallback,
      title: preview || fallback,
      unread: unreadTurns.has(String(turn.id || '')),
    }
  })
  const signature = `${presentationThreadKey()}\u0000${state.language}\u0000${items
    .map((item) => `${item.id}\u0001${item.label}\u0001${item.title}\u0001${item.unread}`).join('\u0000')}`
  if (turns.length < 2) {
    navigator.classList.add('hidden')
    list.innerHTML = ''
    activeTurnNavigatorButton = null
    turnNavigatorButtons = new Map()
    turnNavigatorIds = new Set(items.map((item) => item.id))
    turnNavigatorSignature = signature
    turnNavigatorRenderedKey = threadKey
    finishRender({ buttonCount: turns.length, reused: false })
    return
  }

  if (signature === turnNavigatorSignature && turnNavigatorButtons.size === turns.length) {
    navigator.classList.remove('hidden')
    scheduleTurnNavigatorSync()
    turnNavigatorRenderedKey = threadKey
    finishRender({ buttonCount: turns.length, reused: true })
    return
  }

  list.innerHTML = items.map((item) => {
    return `<button class="turn-nav-item${item.unread ? ' router-unread' : ''}" type="button" data-turn-nav-id="${escapeHtml(item.id)}" aria-label="${escapeHtml(item.label)}${item.unread ? ` · ${t('Unread response')}` : ''}"><span class="turn-nav-title">${escapeHtml(item.title)}</span><span class="turn-nav-indicator" aria-hidden="true"><i></i></span></button>`
  }).join('')
  activeTurnNavigatorButton = null
  turnNavigatorButtons = new Map([...list.querySelectorAll('[data-turn-nav-id]')]
    .map((button) => [button.dataset.turnNavId, button]))
  turnNavigatorIds = new Set(turnNavigatorButtons.keys())
  turnNavigatorSignature = signature
  turnNavigatorRenderedKey = threadKey
  navigator.classList.remove('hidden')
  scheduleTurnNavigatorSync()
  finishRender({ buttonCount: turns.length, reused: false })
}

function scheduleTurnNavigatorSync() {
  if (turnNavigatorFrame != null) return
  turnNavigatorFrame = requestAnimationFrame(syncTurnNavigator)
}

function syncTurnNavigator() {
  turnNavigatorFrame = null
  const navigator = $('#turn-navigator')
  if (navigator.classList.contains('hidden')) return
  const transcript = $('#transcript')
  const transcriptRect = transcript.getBoundingClientRect()
  const marker = transcriptRect.top + Math.min(transcript.clientHeight * 0.28, 160)
  const positions = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .filter((element) => turnNavigatorIds.has(element.dataset.turnId))
    .map((element) => ({ id: element.dataset.turnId, top: element.getBoundingClientRect().top }))
  const atBottom = transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 8
  setActiveTurnNavigator(activeTurnAtMarker(positions, marker, atBottom))
}

function setActiveTurnNavigator(turnId) {
  const list = $('#turn-navigator-list')
  const activeButton = turnNavigatorButtons.get(String(turnId || '')) || null
  if (activeTurnNavigatorButton && activeTurnNavigatorButton !== activeButton) {
    activeTurnNavigatorButton.classList.remove('active')
    activeTurnNavigatorButton.removeAttribute('aria-current')
  }
  if (activeButton && activeTurnNavigatorButton !== activeButton) {
    activeButton.classList.add('active')
    activeButton.setAttribute('aria-current', 'true')
  }
  activeTurnNavigatorButton = activeButton
  if (!activeButton) return
  if (activeButton.offsetTop < list.scrollTop) list.scrollTop = activeButton.offsetTop
  else if (activeButton.offsetTop + activeButton.offsetHeight > list.scrollTop + list.clientHeight) {
    list.scrollTop = activeButton.offsetTop + activeButton.offsetHeight - list.clientHeight
  }
}

function handleTurnNavigatorClick(event) {
  const button = event.target.closest('[data-turn-nav-id]')
  if (!button) return
  navigateTranscriptTurn(button.dataset.turnNavId)
}

function navigateTranscriptTurn(turnId) {
  const key = presentationThreadKey()
  const transcript = $('#transcript')
  if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
  transcriptPresentationCache.setScrollState(key, null)
  transcriptScrollFollower.pause()
  transcriptPresentationCache.showTurn(key, state.model, turnId)
  let target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .find((turn) => turn.dataset.turnId === turnId)
  if (!target) {
    renderTranscript()
    target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
      .find((turn) => turn.dataset.turnId === turnId)
  }
  if (!target) {
    transcriptScrollFollower.reset()
    followTranscriptOutput()
    return
  }
  const top = transcript.scrollTop + target.getBoundingClientRect().top - transcript.getBoundingClientRect().top - 16
  setActiveTurnNavigator(turnId)
  transcript.scrollTop = Math.max(0, top)
  transcriptCaptureSuppressedKeys.delete(key)
  captureTranscriptViewState()
}

function captureRouterReadingPosition() {
  const transcript = $('#transcript'), top = transcript.getBoundingClientRect().top
  const anchor = [...transcript.querySelectorAll('.turn[data-turn-id]')].find(node => node.getBoundingClientRect().bottom > top)
  return { turnId: anchor?.dataset.turnId || '', offset: anchor ? anchor.getBoundingClientRect().top - top : 0, scrollTop: transcript.scrollTop }
}

function showRouterResponse(turnId, response = true) {
  navigateTranscriptTurn(turnId)
  if (response) {
    const section = $('#transcript').querySelector(`.turn[data-turn-id="${CSS.escape(turnId)}"]`)
    const target = section?.querySelector('.router-target-response .message.agent .markdown-body') || section?.querySelector('.router-card')
    if (target) {
      const transcript = $('#transcript')
      transcript.scrollTop += target.getBoundingClientRect().top - transcript.getBoundingClientRect().top - 16
      captureTranscriptViewState()
    }
  }
}

function queueStreamingItemPatch(params = {}) {
  const key = `${params.turnId || ''}:${params.itemId || ''}`
  dirtyStreamItems.set(key, { turnId: params.turnId, itemId: params.itemId })
  if (transcriptFrame != null) return
  transcriptFrame = requestAnimationFrame(flushStreamingItemPatches)
}

function flushStreamingItemPatches() {
  transcriptFrame = null
  let needsFullRender = false
  for (const identity of dirtyStreamItems.values()) {
    if (!patchStreamingItem(identity.turnId, identity.itemId)) needsFullRender = true
  }
  dirtyStreamItems.clear()
  if (needsFullRender) renderTranscript()
  else followTranscriptOutput()
}

function modelItem(turnId, itemId) {
  const turn = state.model.turns.find((candidate) => candidate.id === turnId)
  return turn?.items?.find((candidate) => candidate.id === itemId) || null
}

function renderedItem(turnId, itemId) {
  const container = $('#transcript')
  const key = JSON.stringify([turnId, itemId])
  const cached = transcriptItemNodes.get(key)
  if (cached && container.contains(cached)) return cached
  const element = container.querySelector(`[data-turn-id="${CSS.escape(String(turnId || ''))}"][data-item-id="${CSS.escape(String(itemId || ''))}"]`)
  if (transcriptItemNodes.size >= 2048) transcriptItemNodes.clear()
  if (element) transcriptItemNodes.set(key, element)
  else transcriptItemNodes.delete(key)
  return element
}

function patchStreamingItem(turnId, itemId) {
  transcriptDom.invalidate(turnId)
  const item = modelItem(turnId, itemId)
  const element = renderedItem(turnId, itemId)
  if (!item) return false
  transcriptPresentationCache.invalidateTurn(presentationThreadKey(), turnId)
  if (element && (item.type === 'agentMessage' || item.type === 'plan')) {
    const body = element.querySelector('.markdown-body')
    if (!body) return false
    body.classList.add('streaming-markdown')
    body.textContent = item.type === 'agentMessage' ? sessionMapVisibleText(item.text) : item.text || ''
    return true
  }
  if (item.type === 'commandExecution') {
    if (item.source === 'userShell') {
      const output = element?.querySelector('pre')
      if (!output) return false
      output.textContent = item.aggregatedOutput || ''
      return true
    }
    const block = activityBlocksForTurn(turnId)
      .find((candidate) => candidate.sourceItemIds.includes(String(itemId || '')))
    const activity = renderedActivity(turnId, block?.id)
    if (activity?.open) hydrateActivityDetails(activity, { force: true })
    return true
  }
  return replaceRenderedTurn(turnId)
}

function replaceCompletedItem(params = {}) {
  const itemId = params.item?.id || params.itemId
  dirtyStreamItems.delete(`${params.turnId || ''}:${itemId || ''}`)
  transcriptPresentationCache.invalidateTurn(presentationThreadKey(), params.turnId)
  if (!replaceRenderedTurn(params.turnId)) renderTranscript()
  else sessionResources.sync()
}

function renderedActivity(turnId, activityId = '') {
  return $('#transcript').querySelector(`.work-activity[data-turn-id="${CSS.escape(String(turnId || ''))}"]${activityId ? `[data-activity-id="${CSS.escape(String(activityId))}"]` : ''}`)
}

function replaceRenderedTurn(turnId) {
  const section = $('#transcript').querySelector(`.turn[data-turn-id="${CSS.escape(String(turnId || ''))}"]`)
  if (!section) return false
  const openActivityIds = [...section.querySelectorAll('.work-activity[open]')]
    .map((activity) => activity.dataset.activityId)
  const entry = transcriptPresentationCache.updateTurn(presentationThreadKey(), state.model, turnId)
  const presentation = entry.turns.get(String(turnId || ''))?.presentation
  if (!presentation) return false
  transcriptDom.invalidate(turnId)
  transcriptItemNodes.clear()
  const template = document.createElement('template')
  template.innerHTML = renderTurn(presentation, entry.orderedIds.indexOf(String(turnId || '')), { openActivityIds })
  section.replaceWith(template.content)
  observeTranscriptContent()
  bindActivityDetails()
  reviewNotes.renderCommentMarkers()
  for (const activityId of openActivityIds) {
    const activity = renderedActivity(turnId, activityId)
    if (activity) hydrateActivityDetails(activity)
  }
  scheduleTurnNavigatorRender()
  followTranscriptOutput()
  return true
}

function renderTurn(presentation, index, { openActivityIds = [] } = {}) {
  if (!presentation) return ''
  const content = presentation.blocks.map((block) => renderPresentationBlock(block, presentation.id, {
    openActivity: openActivityIds.includes(block.id),
    forkable: !isArchivedPreview() && isTurnForkable(presentation.source),
  })).join('')
  const placeholder = shouldShowTurnPlaceholder(presentation)
    ? `<div class="work-placeholder"><span class="message-track-mark" aria-hidden="true">${conversationTrackIcon('working')}</span><span>${t('{backend} is preparing this turn…', { backend: currentBackend().name })}</span></div>`
    : ''
  return `<section class="turn" data-turn-id="${escapeHtml(presentation.id)}" data-turn-index="${index}">${content}${placeholder}</section>`
}

function renderPresentationBlock(block, turnId, options = {}) {
  if (block.type === 'command') return renderItem(block.item, turnId)
  if (block.type === 'user') return renderItem(block.item, turnId)
  if (block.type === 'assistant') return renderItem(block.item, turnId, { forkable: options.forkable, sourceRef: options.sourceRef })
  if (block.type === 'activity') return renderActivity(block, turnId, options)
  if (block.type === 'error') return `<div class="turn-error" role="alert"><span class="message-track-mark turn-error-mark" aria-hidden="true">${conversationTrackIcon('failed')}</span><div class="turn-error-content"><strong>${t('Execution failed')}</strong><span>${escapeHtml(block.message)}</span></div></div>`
  return ''
}

function renderRouterTargetTurn(turn, sourceRef) {
  const open = new Set([...document.querySelectorAll(`[data-router-source="${CSS.escape(sourceRef.key)}"] .work-activity[open][data-turn-id="${CSS.escape(String(turn.id))}"]`)].map(node => node.dataset.activityId))
  // Reuse the bounded LRU budget instead of retaining another unbounded HTML cache.
  // Mutable native items, favorites, locale and disclosure state all affect HTML.
  const favoriteFlags = (turn.items || []).filter(item => item.type === 'agentMessage' || item.type === 'plan')
    .map(item => Boolean(reviewNotes.favoriteForSource(sourceRef.backend, sourceRef.id, turn.id, item.id)))
  const cacheKey = turn.status === 'completed'
    ? `router-result\u0000${JSON.stringify([sourceRef, getLocale(), turn, [...open].sort(), favoriteFlags])}` : ''
  if (cacheKey) {
    const cached = readMarkdownRenderCache(cacheKey)
    if (cached != null) return cached
  }
  const rendered = presentRoutedTurn(turn).blocks.map(block => renderPresentationBlock(block, turn.id, {
    sourceRef, forkable: false, openActivity: open.has(block.id),
  })).join('')
  if (cacheKey && (cacheKey.length + rendered.length) * 2 <= MAX_MARKDOWN_RENDER_CACHE_BYTES) {
    writeMarkdownRenderCache(cacheKey, rendered, rendered.length * 2)
  }
  return rendered
}

function conversationTrackIcon(kind) {
  if (kind === 'working') return '<span class="track-icon activity-spinner"></span>'
  const shapes = {
    question: '<path d="m6.2 4.7 4.5 4.3-4.5 4.3"></path>',
    response: '<circle cx="9" cy="9" r="4.6"></circle>',
    completed: '<path d="m4.5 9.1 3 3.1 6-6.2"></path>',
    failed: '<path d="M9 2.8 16 15H2Z"></path><path d="M9 6.5v4.2"></path><path d="M9 13v.1"></path>',
  }
  return `<svg class="track-icon track-icon-${kind}" viewBox="0 0 18 18" focusable="false">${shapes[kind] || shapes.response}</svg>`
}

function renderActivity(block, turnId, { openActivity = false } = {}) {
  const summary = activitySummaryParts(block.summary)
  const stateClass = block.active ? ' active' : block.summary.failures ? ' failed' : ''
  const title = block.active ? t('Working') : t('Worked')
  const statusIcon = conversationTrackIcon(block.active ? 'working' : block.summary.failures ? 'failed' : 'completed')
  const stage = block.latestStage ? `<span class="activity-stage">${escapeHtml(block.latestStage)}</span>` : ''
  const metrics = summary.length ? `<span class="activity-metrics">${summary.map(escapeHtml).join('<i>·</i>')}</span>` : ''
  return `<details class="work-activity${stateClass}" data-turn-id="${escapeHtml(turnId)}" data-activity-id="${escapeHtml(block.id)}"${openActivity ? ' open' : ''}>
    <summary><span class="activity-leading" aria-hidden="true">${statusIcon}</span><strong class="activity-title">${title}</strong>${stage}${metrics}<span class="activity-chevron" aria-hidden="true">›</span></summary>
    <div class="activity-detail-body" data-activity-empty="true"></div>
  </details>`
}

function activitySummaryParts(summary = {}) {
  const parts = []
  const explored = (summary.reads || 0) + (summary.searches || 0) + (summary.lists || 0)
  if (explored) parts.push(t('Explored {count}', { count: explored }))
  if (summary.commands) parts.push(t('Ran {count} commands', { count: summary.commands }))
  if (summary.tools) parts.push(t('Called {count} tools', { count: summary.tools }))
  if (summary.webSearches) parts.push(t('Searched the web {count} times', { count: summary.webSearches }))
  if (summary.changedFiles) parts.push(t('Changed {count} files', { count: summary.changedFiles }))
  if (summary.failures) parts.push(t('{count} failed', { count: summary.failures }))
  return parts
}

function bindActivityDetails() {
  $$('.work-activity').forEach((details) => {
    if (details.dataset.activityBound === 'true') return
    details.dataset.activityBound = 'true'
    details.addEventListener('toggle', () => {
      if (details.open) hydrateActivityDetails(details)
    })
    if (details.open) hydrateActivityDetails(details)
  })
}

function activityPresentationForTurn(turnId, source = null) {
  if (source) {
    const turn = source.model?.turns?.find(turn => String(turn.id) === String(turnId))
    return turn ? presentRoutedTurn(turn) : null
  }
  return transcriptPresentationCache.updateTurn(presentationThreadKey(), state.model, turnId)
    .turns.get(String(turnId || ''))?.presentation || null
}

function activityBlocksForTurn(turnId) {
  return presentationActivityBlocks(activityPresentationForTurn(turnId))
}

function activityBlockForTurn(turnId, activityId) {
  return activityBlocksForTurn(turnId)
    .find((block) => block.id === String(activityId || '')) || null
}

function hydrateActivityDetails(details, { force = false } = {}) {
  const body = details.querySelector('.activity-detail-body')
  if (!body || (!force && body.dataset.activityEmpty !== 'true')) return
  const source = threadRouter.sourceContext(details)
  const block = source
    ? presentationActivityBlocks(activityPresentationForTurn(details.dataset.turnId, source)).find(block => block.id === details.dataset.activityId)
    : activityBlockForTurn(details.dataset.turnId, details.dataset.activityId)
  if (!block) return
  body.innerHTML = (block.displayEntries || block.entries).map(renderActivityEntry).join('')
    + `<button class="activity-log-button" type="button" data-activity-log="${escapeHtml(details.dataset.turnId)}">${t('View full activity log')}</button>`
  body.dataset.activityEmpty = 'false'
}

function renderActivityEntry(entry) {
  const item = entry.item || {}
  const status = escapeHtml(item.status || entry.status || 'completed')
  if (entry.kind === 'reasoning') {
    const stage = reasoningStage(item)
    return stage ? `<div class="activity-entry reasoning-entry"><span>◆</span><p>${escapeHtml(stage)}</p></div>` : ''
  }
  if (entry.kind === 'progress') {
    return `<div class="activity-entry progress-entry"><span>•</span><p>${escapeHtml(sessionMapVisibleText(item.text || ''))}</p></div>`
  }
  if (entry.kind === 'command') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    const preview = activityOutputPreview(item.aggregatedOutput || '')
    return `<div class="activity-entry command-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('Active')}</strong><code>${escapeHtml(command)}</code></header>${renderOutputPreview(preview)}</div>`
  }
  if (entry.kind === 'change') {
    const rows = (item.changes || []).map((change) => `<li><span>${escapeHtml(change.kind || 'update')}</span><code>${escapeHtml(change.path || '')}</code></li>`).join('')
    return `<div class="activity-entry change-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('File changes · {count} files', { count: (item.changes || []).length })}</strong></header><ul>${rows}</ul></div>`
  }
  if (entry.kind === 'plan') {
    const rows = (item.plan || []).map((step) => `<li class="${escapeHtml(step.status || '')}">${escapeHtml(step.step || '')}</li>`).join('')
    return `<div class="activity-entry plan-entry"><header><span>☷</span><strong>${t('Execution plan')}</strong></header><ol class="plan-list">${rows}</ol></div>`
  }
  if (entry.kind === 'search') {
    return `<div class="activity-entry tool-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('Web search')}</strong><span>${escapeHtml(item.query || '')}</span></header></div>`
  }
  if (entry.kind === 'tool') {
    const label = `${item.server || 'Tool'} · ${item.tool || item.type || 'tool'}`
    const preview = activityOutputPreview(valueText(item.result || item.error || ''))
    return `<div class="activity-entry tool-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${escapeHtml(label)}</strong></header>${renderOutputPreview(preview)}</div>`
  }
  if (entry.kind === 'system') return `<div class="activity-entry system-entry"><span>•</span><p>${item.type === 'contextCompaction' ? t('Codex compacted earlier conversation context.') : escapeHtml(item.reason || item.type || '')}</p></div>`
  return `<div class="activity-entry unknown-entry"><span>•</span><p>${escapeHtml(item.type || t('Unknown'))}</p></div>`
}

function activityEntryIcon(status) {
  if (status === 'failed') return '×'
  if (status === 'inProgress') return '<i class="activity-spinner"></i>'
  return '✓'
}

function renderOutputPreview(preview) {
  if (!preview?.lines?.length) return ''
  const lines = [...preview.lines]
  if (preview.omitted && preview.splitAt != null) lines.splice(preview.splitAt, 0, t('… {count} lines omitted', { count: preview.omitted }))
  return `<pre>${escapeHtml(lines.join('\n'))}</pre>`
}

function renderItem(item, turnId, { forkable = false, sourceRef = null } = {}) {
  const type = item?.type || 'unknown'
  const attrs = `data-turn-id="${escapeHtml(turnId || '')}" data-item-id="${escapeHtml(item?.id || '')}"`
  if (type === 'userMessage') {
    const text = textFromUserContent(item.content)
    const images = renderUserMessageImages(item.content)
    return `<div class="message user" ${attrs}><span class="message-track-mark user-track-mark" aria-hidden="true">${conversationTrackIcon('question')}</span><div class="message-content">${images}${text ? `<div>${escapeHtml(text)}</div>` : (!images ? escapeHtml(t('(non-text input)')) : '')}</div></div>`
  }
  if (type === 'agentMessage' || type === 'plan') {
    const favorite = reviewNotes.favoriteForSource(sourceRef?.backend || state.backend, sourceRef?.id || state.selectedId, turnId, item.id)
    const favoriteLabel = favorite ? 'Favorited; click to view' : 'Favorite this response'
    const forkAction = forkable
      ? `<button class="message-fork-button" type="button" data-fork-turn="${escapeHtml(turnId || '')}" title="${t('Fork from here')}" aria-label="${t('Fork from here')}"><svg viewBox="0 0 18 18" aria-hidden="true"><circle cx="4.25" cy="4" r="1.65"></circle><circle cx="4.25" cy="14" r="1.65"></circle><circle cx="13.75" cy="9" r="1.65"></circle><path d="M4.25 5.65v6.7M5.9 4h2.15a4.05 4.05 0 0 1 4.05 4.05V9"></path></svg><b>${t('Fork from here')}</b></button>`
      : ''
    return `<div class="message agent${favorite ? ' favorited' : ''}" ${attrs}>
      <span class="message-track-mark agent-track-mark" aria-hidden="true">${conversationTrackIcon('response')}</span>
      <div class="message-content"><div class="markdown-body">${renderMarkdown(type === 'agentMessage' ? sessionMapVisibleText(item.text) : item.text || '')}</div>
      <div class="message-actions"><button class="message-copy-button" type="button" data-copy-message="${escapeHtml(item.id || '')}" title="${t('Copy content')}" aria-label="${t('Copy content')}"><svg viewBox="0 0 18 18" aria-hidden="true"><rect x="2.75" y="2.75" width="8.5" height="10" rx="1.5"></rect><rect x="6.75" y="5.25" width="8.5" height="10" rx="1.5"></rect></svg><b>${t('Copy')}</b></button><button class="message-favorite-button${favorite ? ' active' : ''}" type="button" data-favorite-message="${escapeHtml(item.id || '')}" title="${favoriteLabel}" aria-label="${favoriteLabel}" aria-pressed="${Boolean(favorite)}"><svg viewBox="0 0 18 18" aria-hidden="true"><path d="m9 2.8 2.02 4.09 4.51.66-3.27 3.18.77 4.5L9 13.11l-4.03 2.12.77-4.5-3.27-3.18 4.51-.66Z"></path></svg><b>${favorite ? 'Favorited' : 'Favorites'}</b></button>${forkAction}${sourceRef ? threadRouter.renderSourceActions(sourceRef) : ''}</div></div>
    </div>`
  }
  if (type === 'reasoning') {
    const summary = arrayText(item.summary) || arrayText(item.content) || t('{backend} is reasoning…', { backend: currentBackend().name })
    return `<details class="reasoning" ${attrs} open><summary>Reasoning summary</summary><div class="markdown-body compact-markdown">${renderMarkdown(summary)}</div></details>`
  }
  if (type === 'commandExecution') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    return `<article class="item-card" ${attrs}><header><span>${t('Command')} · ${escapeHtml(command)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header><pre>${escapeHtml(item.aggregatedOutput || '')}</pre></article>`
  }
  if (type === 'fileChange') {
    const changes = (item.changes || []).map((change) => `${change.kind || 'update'} ${change.path || ''}\n${change.diff || ''}`).join('\n\n')
    return `<article class="item-card" ${attrs}><header><span>${t('File changes · {count} files', { count: (item.changes || []).length })}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header><pre>${escapeHtml(changes || t('Waiting for diff…'))}</pre></article>`
  }
  if (type === 'planUpdate') {
    const rows = (item.plan || []).map((step) => `<li class="${escapeHtml(step.status || '')}">${escapeHtml(step.step || '')}</li>`).join('')
    return `<article class="item-card" ${attrs}><header><span>Execution plan</span><span data-no-i18n>${escapeHtml(item.explanation || '')}</span></header><ol class="plan-list" data-no-i18n>${rows}</ol></article>`
  }
  if (type === 'mcpToolCall' || type === 'collabToolCall' || type === 'webSearch') {
    const label = type === 'webSearch' ? `${t('Web search')} · ${item.query || ''}` : `${item.server || 'Tool'} · ${item.tool || type}`
    const detail = item.result || item.error || item.arguments || item.results || ''
    return `<article class="item-card" ${attrs}><header><span>${escapeHtml(label)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${detail ? `<pre>${escapeHtml(valueText(detail))}</pre>` : ''}</article>`
  }
  if (type === 'contextCompaction') return `<div class="reasoning" ${attrs}>Codex compacted earlier conversation context.</div>`
  return `<article class="item-card" ${attrs}><header><span>${escapeHtml(type)}</span></header><pre>${escapeHtml(valueText(item))}</pre></article>`
}

function renderUserMessageImages(content) {
  const images = userImagesFromContent(content)
  if (!images.length) return ''
  const rows = images.map((image) => image.kind === 'url'
    ? `<figure class="message-user-image"><img src="${escapeHtml(image.source)}" alt="${escapeHtml(t(image.label))}" loading="lazy" /><figcaption>${escapeHtml(t(image.label))}</figcaption></figure>`
    : `<figure class="message-user-image local"><figcaption>${escapeHtml(image.label)}</figcaption></figure>`)
  return `<div class="message-user-images">${rows.join('')}</div>`
}

function renderMarkdown(value) {
  const source = String(value || '')
  if (!source) return ''
  const cacheKey = `message\u0000${getLocale()}\u0000${source}`
  const cached = readMarkdownRenderCache(cacheKey)
  if (cached != null) return cached
  const rendered = renderMarkdownHtml(source)
  writeMarkdownRenderCache(cacheKey, rendered, rendered.length * 2)
  return rendered
}

function renderMarkdownDocument(value) {
  const source = String(value || '')
  if (!source) return { html: '', outline: [] }
  const cacheKey = `document\u0000${getLocale()}\u0000${source}`
  const cached = readMarkdownRenderCache(cacheKey)
  if (cached != null) return cached
  const tokens = marked.lexer(source)
  const rendered = {
    html: renderMarkdownHtml(source, { tokens, documentImages: true }),
    outline: extractMarkdownOutline(source, tokens),
  }
  const outlineSize = rendered.outline.reduce((size, item) => size + item.id.length + item.label.length + 64, 0)
  writeMarkdownRenderCache(cacheKey, rendered, (rendered.html.length * 2) + outlineSize)
  return rendered
}

function readMarkdownRenderCache(key) {
  const entry = markdownRenderCache.get(key)
  if (!entry) return null
  markdownRenderCache.delete(key)
  markdownRenderCache.set(key, entry)
  return entry.value
}

function writeMarkdownRenderCache(key, value, size) {
  const previous = markdownRenderCache.get(key)
  if (previous) markdownRenderCacheBytes -= previous.size
  const boundedSize = Math.max(0, Number(size) || 0) + (key.length * 2)
  markdownRenderCache.delete(key)
  markdownRenderCache.set(key, { value, size: boundedSize })
  markdownRenderCacheBytes += boundedSize
  while (markdownRenderCacheBytes > MAX_MARKDOWN_RENDER_CACHE_BYTES && markdownRenderCache.size > 1) {
    const oldestKey = markdownRenderCache.keys().next().value
    const oldest = markdownRenderCache.get(oldestKey)
    markdownRenderCache.delete(oldestKey)
    markdownRenderCacheBytes -= oldest?.size || 0
  }
}

function renderMarkdownHtml(source, { tokens = null, documentImages = false } = {}) {
  const dirty = tokens ? marked.parser(tokens) : marked.parse(source)
  const clean = DOMPurify.sanitize(dirty, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['button', 'form', 'iframe', 'object', 'embed', 'script', 'style'],
    FORBID_ATTR: ['style'],
  })
  const template = document.createElement('template')
  template.innerHTML = clean

  template.content.querySelectorAll('a').forEach((link) => {
    link.dataset.resourceTarget = link.getAttribute('href') || ''
    link.removeAttribute('target')
    link.rel = 'noopener noreferrer'
  })
  if (documentImages) {
    template.content.querySelectorAll('img').forEach((image) => {
      const source = image.getAttribute('src') || ''
      image.dataset.markdownImageSource = source
      image.removeAttribute('src')
      image.removeAttribute('srcset')
      image.loading = 'lazy'
      image.decoding = 'async'
      image.classList.add('markdown-local-image', 'loading')
    })
  }
  template.content.querySelectorAll('input').forEach((input) => {
    if (input.type !== 'checkbox') input.remove()
    else input.disabled = true
  })
  template.content.querySelectorAll('pre > code').forEach((code) => {
    const pre = code.parentElement
    const languageClass = [...code.classList].find((name) => name.startsWith('language-'))
    const language = languageClass ? languageClass.slice('language-'.length) : 'code'
    const block = document.createElement('div')
    block.className = 'markdown-code-block'
    const normalizedLanguage = language.toLowerCase()
    const header = document.createElement('div')
    header.className = 'markdown-code-header'
    const label = document.createElement('span')
    label.textContent = language
    const copy = document.createElement('button')
    copy.className = 'copy-code-button'
    copy.type = 'button'
    copy.textContent = t('Copy')
    pre.replaceWith(block)
    if (normalizedLanguage === 'mermaid') {
      block.classList.add('markdown-mermaid')
      block.dataset.mermaidState = 'pending'
      block.dataset.mermaidGeneration = String(mermaidGeneration)
      const actions = document.createElement('div')
      actions.className = 'markdown-code-actions'
      const sourceToggle = document.createElement('button')
      sourceToggle.className = 'mermaid-source-button'
      sourceToggle.type = 'button'
      sourceToggle.setAttribute('aria-expanded', 'false')
      sourceToggle.textContent = t('Source code')
      actions.append(sourceToggle, copy)
      header.append(label, actions)
      const canvas = document.createElement('div')
      canvas.className = 'markdown-mermaid-canvas'
      canvas.dataset.noI18n = ''
      canvas.setAttribute('role', 'img')
      canvas.setAttribute('aria-label', t('Mermaid diagram'))
      canvas.setAttribute('aria-busy', 'true')
      canvas.textContent = t('Rendering diagram…')
      pre.classList.add('markdown-mermaid-source')
      block.append(header, canvas, pre)
    } else if (normalizedLanguage === 'text' || normalizedLanguage === 'plaintext' || normalizedLanguage === 'txt') {
      block.classList.add('markdown-plain-text')
      block.append(pre)
    } else {
      header.append(label, copy)
      block.append(header, pre)
    }
  })
  template.content.querySelectorAll('table').forEach((table) => {
    const wrapper = document.createElement('div')
    wrapper.className = 'markdown-table-wrap'
    table.replaceWith(wrapper)
    wrapper.append(table)
  })
  return template.innerHTML
}

function hydrateMarkdownImages(file, content) {
  disconnectMarkdownImageObserver()
  const images = [...content.querySelectorAll('img[data-markdown-image-source]')]
  if (!images.length) return

  const load = (image) => {
    artifactMarkdownImageObserver?.unobserve(image)
    loadMarkdownImage(file, image).catch((error) => showMarkdownImageError(image, error))
  }
  if (!globalThis.IntersectionObserver) {
    images.forEach(load)
    return
  }
  artifactMarkdownImageObserver = new IntersectionObserver((entries) => {
    entries.filter((entry) => entry.isIntersecting).forEach((entry) => load(entry.target))
  }, { root: content, rootMargin: '600px 0px' })
  images.forEach((image) => artifactMarkdownImageObserver.observe(image))
}

async function loadMarkdownImage(file, image) {
  if (state.artifact !== file || !image.isConnected) return
  const source = image.dataset.markdownImageSource || ''
  const target = resolveMarkdownImagePath(file, source)
  if (!target) throw new Error(t('Only workspace-local, data, and blob images can be displayed'))
  if (target.embedded) {
    await assignMarkdownImageSource(image, target.embedded)
    return
  }

  file.markdownImageAssets ||= new Map()
  let pending = file.markdownImageAssets.get(target.path)
  if (!pending) {
    pending = gatewayFetch('/studio/review-image', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ root: file.root, path: target.path }),
    }).then(async (response) => {
      if (!response.ok) {
        const result = await response.json().catch(() => null)
        throw new Error(result?.error?.message || `HTTP ${response.status}`)
      }
      const blob = await response.blob()
      if (!blob.type.startsWith('image/')) throw new Error(t('The image response format is invalid'))
      const url = URL.createObjectURL(blob)
      file.markdownImageObjectUrls ||= new Set()
      file.markdownImageObjectUrls.add(url)
      return url
    })
    file.markdownImageAssets.set(target.path, pending)
  }
  const url = await pending
  if (state.artifact !== file || !image.isConnected) return
  await assignMarkdownImageSource(image, url)
}

function assignMarkdownImageSource(image, source) {
  return new Promise((resolve, reject) => {
    image.addEventListener('load', () => {
      image.classList.remove('loading')
      resolve()
    }, { once: true })
    image.addEventListener('error', () => reject(new Error(t('Unable to decode image'))), { once: true })
    image.src = source
  })
}

function showMarkdownImageError(image, error) {
  if (!image.isConnected) return
  const source = image.dataset.markdownImageSource || ''
  const fallback = document.createElement('span')
  fallback.className = 'markdown-image-error'
  fallback.setAttribute('role', 'note')
  fallback.dataset.noI18n = ''
  fallback.textContent = `${t('Image unavailable')}: ${image.alt || source}`
  fallback.title = error?.message || String(error || '')
  image.replaceWith(fallback)
}

function disconnectMarkdownImageObserver() {
  artifactMarkdownImageObserver?.disconnect()
  artifactMarkdownImageObserver = null
}

function disposeMarkdownImageAssets(file) {
  disconnectMarkdownImageObserver()
  file?.markdownImageObjectUrls?.forEach((url) => URL.revokeObjectURL(url))
  file?.markdownImageObjectUrls?.clear()
  file?.markdownImageAssets?.clear()
}

function startMermaidRendering() {
  if (mermaidObserver || !globalThis.MutationObserver) return
  mermaidObserver = new MutationObserver((records) => {
    records.forEach((record) => record.addedNodes.forEach(scanMermaidNode))
  })
  mermaidObserver.observe(document.body, { subtree: true, childList: true })
  scanMermaidNode(document.body)
}

function scanMermaidNode(node) {
  if (node.nodeType !== Node.ELEMENT_NODE && node.nodeType !== Node.DOCUMENT_FRAGMENT_NODE) return
  if (node.matches?.('.markdown-mermaid[data-mermaid-state="pending"]')) queueMermaidBlock(node)
  node.querySelectorAll?.('.markdown-mermaid[data-mermaid-state="pending"]').forEach(queueMermaidBlock)
}

function queueMermaidBlock(block) {
  if (block.dataset.mermaidState !== 'pending') return
  block.dataset.mermaidState = 'queued'
  const generation = Number(block.dataset.mermaidGeneration || mermaidGeneration)
  mermaidRenderChain = mermaidRenderChain
    .then(() => renderMermaidBlock(block, generation))
    .catch(reportClientError)
}

async function renderMermaidBlock(block, generation) {
  if (!block.isConnected || generation !== mermaidGeneration) return
  const source = block.querySelector('.markdown-mermaid-source code')?.textContent || ''
  if (source.length > MAX_MERMAID_SOURCE_CHARS) {
    showMermaidError(block, t('The diagram is too large; its source is shown.'))
    return
  }
  const mermaid = globalThis.mermaid
  if (!mermaid?.initialize || !mermaid?.render) {
    showMermaidError(block, t('Unable to render diagram'))
    return
  }
  const config = mermaidInitializeConfig(state.mermaid, {
    dark: state.theme === 'dark',
    fontFamily: state.typography.contentFontFamily,
  })
  const configSignature = JSON.stringify(config)
  if (mermaidInitializedConfig !== configSignature) {
    mermaid.initialize(config)
    mermaidInitializedConfig = configSignature
  }
  const canvas = block.querySelector('.markdown-mermaid-canvas')
  if (!canvas) return
  block.dataset.mermaidState = 'rendering'
  canvas.setAttribute('aria-busy', 'true')
  canvas.textContent = t('Rendering diagram…')
  const diagramId = `studio-mermaid-${++mermaidRenderSequence}`
  try {
    const result = await mermaid.render(diagramId, source)
    if (!block.isConnected || generation !== mermaidGeneration) return
    const cleanSvg = DOMPurify.sanitize(result.svg, {
      USE_PROFILES: { html: true, svg: true, svgFilters: true },
      FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form'],
    })
    const template = document.createElement('template')
    template.innerHTML = cleanSvg
    if (!template.content.querySelector('svg')) throw new Error('Mermaid did not produce an SVG')
    canvas.replaceChildren(template.content.cloneNode(true))
    canvas.setAttribute('aria-busy', 'false')
    block.dataset.mermaidState = 'rendered'
    result.bindFunctions?.(canvas)
    requestAnimationFrame(() => {
      if (block.closest('#transcript')) followTranscriptOutput()
      scheduleTurnNavigatorSync()
    })
  } catch (error) {
    document.getElementById(diagramId)?.remove()
    showMermaidError(block, t('Unable to render diagram'))
    reportClientError(error)
  }
}

function showMermaidError(block, message) {
  const canvas = block.querySelector('.markdown-mermaid-canvas')
  if (canvas) {
    canvas.textContent = message
    canvas.setAttribute('aria-busy', 'false')
  }
  block.dataset.mermaidState = 'error'
  setMermaidSourceVisible(block, true)
}

function setMermaidSourceVisible(block, visible) {
  block.classList.toggle('show-source', visible)
  const button = block.querySelector('.mermaid-source-button')
  if (!button) return
  button.setAttribute('aria-expanded', String(visible))
  button.textContent = t(visible ? 'Hide source' : 'Source code')
}

function resetMermaidRendering() {
  if (!mermaidObserver) return
  mermaidGeneration += 1
  mermaidInitializedConfig = ''
  document.querySelectorAll('.markdown-mermaid').forEach((block) => {
    block.dataset.mermaidState = 'pending'
    block.dataset.mermaidGeneration = String(mermaidGeneration)
    const canvas = block.querySelector('.markdown-mermaid-canvas')
    if (canvas) {
      canvas.setAttribute('aria-busy', 'true')
      canvas.textContent = t('Rendering diagram…')
    }
    queueMermaidBlock(block)
  })
}

async function handleMarkdownActionClick(event) {
  const sourceButton = event.target.closest('.mermaid-source-button')
  if (sourceButton) {
    const block = sourceButton.closest('.markdown-mermaid')
    if (block) {
      setMermaidSourceVisible(block, !block.classList.contains('show-source'))
      if (state.artifactSearch.trim() && block.closest('#artifact-content')) applyArtifactSearchHighlights()
    }
    return
  }
  const button = event.target.closest('.copy-code-button')
  if (!button) return
  const code = button.closest('.markdown-code-block')?.querySelector('code')
  if (!code) return
  try {
    await navigator.clipboard.writeText(code.textContent || '')
    const original = button.textContent
    button.textContent = t('Copied')
    setTimeout(() => { if (button.isConnected) button.textContent = original }, 1400)
  } catch {
    toast(t('Unable to copy code'), 'error')
  }
}

async function handleTranscriptClick(event) {
  const supervisionButton = event.target.closest?.('[data-supervision-action]')
  if (supervisionButton) {
    event.preventDefault()
    const ref = { backend: supervisionButton.dataset.backend, id: supervisionButton.dataset.thread }
    try {
      if (supervisionButton.dataset.supervisionAction === 'stop') turnSupervision.stop(ref)
      else {
        enableTurnSupervision(ref, supervisionButton.dataset.turn)
      }
    } catch (error) { showError(error) }
    return
  }
  if (event.target.closest?.('[data-router-reply], [data-router-tool]')) {
    event.preventDefault()
    await threadRouter.handleAction(event.target)
    return
  }
  const resourceLink = event.target.closest('.markdown-body a[data-resource-target]')
  if (resourceLink) {
    const target = resourceLink.dataset.resourceTarget || ''
    if (!target || target.startsWith('#')) return
    event.preventDefault()
    if (/^https?:\/\//iu.test(target)) {
      await openBrowserUrl(target)
      return
    }
    const document = resourceLink.closest('#artifact-content') ? state.artifact : null
    const file = resolveMarkdownFileLink(target, document?.path || '')
    if (!file) return
    const source = threadRouter.sourceContext(resourceLink)
    if (source && !source.thread?.cwd) throw new Error(t('The target session no longer exists.'))
    const openLinkedArtifact = () => openArtifact(
      { root: document?.root || source?.thread?.cwd || selectedThread()?.cwd, path: file.path, sourceSessionKey: document?.sourceSessionKey || source?.key || '' },
      { returnTool: document?.returnTool || (state.activeRightWorkspace === 'resources' ? 'resources' : '') },
    )
    if (event.currentTarget === $('#transcript')) {
      let opening
      preserveTranscriptLayout(() => { opening = openLinkedArtifact() })
      await opening
    } else {
      await openLinkedArtifact()
    }
    if (file.line) jumpArtifactToLine(file.line, file.column)
    return
  }
  const earlier = event.target.closest('[data-load-earlier]')
  if (earlier) {
    const transcript = $('#transcript')
    beginTranscriptProgrammaticNavigation()
    const previousHeight = transcript.scrollHeight
    const previousTop = transcript.scrollTop
    transcriptPresentationCache.showEarlier(presentationThreadKey(), state.model, 20)
    renderTranscript({ preserveScroll: true, previousHeight, previousTop })
    return
  }
  const latest = event.target.closest('[data-jump-latest]')
  if (latest) {
    beginTranscriptFollowingLatest()
    renderTranscript()
    return
  }
  const activityLog = event.target.closest('[data-activity-log]')
  if (activityLog) {
    openActivityLog(activityLog.dataset.activityLog, threadRouter.sourceContext(activityLog))
    return
  }
  const routerTarget = event.target.closest('[data-router-target]')
  if (routerTarget) {
    await threadRouter.openTarget(routerTarget)
    return
  }
  const forkButton = event.target.closest('[data-fork-turn]')
  if (forkButton) {
    if (isArchivedPreview()) return
    await forkThread(forkButton.dataset.forkTurn, forkButton)
    return
  }
  const favoriteButton = event.target.closest('[data-favorite-message]')
  if (favoriteButton) {
    const element = favoriteButton.closest('[data-turn-id][data-item-id]')
    if (!element) return
    const existing = reviewNotes.favoriteForSource(
      threadRouter.sourceContext(element)?.backend || state.backend,
      threadRouter.sourceContext(element)?.id || state.selectedId,
      element.dataset.turnId,
      element.dataset.itemId,
    )
    if (existing) await reviewNotes.openFavoriteDetail(existing.id)
    else reviewNotes.openFavoriteForMessage(element.dataset.turnId, element.dataset.itemId, threadRouter.sourceContext(element))
    return
  }
  const copyMessageButton = event.target.closest('[data-copy-message]')
  if (copyMessageButton) {
    const element = copyMessageButton.closest('[data-turn-id][data-item-id]')
    const source = threadRouter.sourceContext(element)
    const item = source ? source.model?.turns?.find(turn => String(turn.id) === element.dataset.turnId)?.items?.find(item => String(item.id) === element.dataset.itemId) : element && modelItem(element.dataset.turnId, element.dataset.itemId)
    if (!item) return
    const content = item.type === 'agentMessage' ? sessionMapVisibleText(item.text || '') : item.text || ''
    try {
      await navigator.clipboard.writeText(content)
      copyMessageButton.classList.add('copied')
      toast(t('Copied'))
      setTimeout(() => { if (copyMessageButton.isConnected) copyMessageButton.classList.remove('copied') }, 1400)
    } catch {
      toast(t('Unable to copy response'), 'error')
    }
    return
  }
}

function openActivityLog(turnId, source = null) {
  const entries = presentationActivityEntries(activityPresentationForTurn(turnId, source))
  if (!entries.length) return
  activityLogContext = { turnId: String(turnId || ''), entries }
  $('#activity-log-title').textContent = t('Full activity log')
  $('#activity-log-subtitle').textContent = t('Raw details load per item without slowing the main chat.')
  $('#activity-log-content').innerHTML = entries.map((entry, index) => {
    const item = entry.item || {}
    const label = activityRawLabel(entry)
    return `<details class="activity-raw-item" data-activity-entry-index="${index}"><summary><span>${activityEntryIcon(item.status)}</span><strong>${escapeHtml(label)}</strong><small>${escapeHtml(statusLabel(item.status || entry.status))}</small></summary><div class="activity-raw-body" data-raw-empty="true"></div></details>`
  }).join('') || `<div class="command-empty">${t('No activity recorded')}</div>`
  $$('#activity-log-content .activity-raw-item').forEach((details) => details.addEventListener('toggle', () => {
    if (!details.open) return
    const body = details.querySelector('.activity-raw-body')
    if (!body || body.dataset.rawEmpty !== 'true') return
    const entry = activityLogContext?.entries[Number(details.dataset.activityEntryIndex)]
    if (!entry) return
    body.innerHTML = renderRawActivityEntry(entry)
    body.dataset.rawEmpty = 'false'
  }))
  $('#activity-log-dialog').showModal()
}

function activityRawLabel(entry) {
  const item = entry.item || {}
  if (entry.kind === 'command') return Array.isArray(item.command) ? item.command.join(' ') : item.command || t('Command')
  if (entry.kind === 'reasoning') return reasoningStage(item) || t('Reasoning summary')
  if (entry.kind === 'progress') return truncateForDisplay(item.text || '', 100)
  if (entry.kind === 'change') return t('File changes · {count} files', { count: item.changes?.length || 0 })
  if (entry.kind === 'search') return `${t('Web search')} · ${item.query || ''}`
  if (entry.kind === 'tool') return `${item.server || 'Tool'} · ${item.tool || item.type || 'tool'}`
  if (entry.kind === 'plan') return t('Execution plan')
  return item.type || t('Unknown')
}

function renderRawActivityEntry(entry) {
  const item = entry.item || {}
  if (entry.kind === 'command') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    return `<pre class="activity-raw-code"><code>$ ${escapeHtml(command)}${item.aggregatedOutput ? `\n\n${escapeHtml(item.aggregatedOutput)}` : ''}</code></pre>`
  }
  if (entry.kind === 'reasoning') {
    const content = arrayText(item.summary) || arrayText(item.content) || ''
    return `<div class="markdown-body compact-markdown">${renderMarkdown(content)}</div>`
  }
  if (entry.kind === 'progress') return `<div class="markdown-body compact-markdown">${renderMarkdown(sessionMapVisibleText(item.text || ''))}</div>`
  if (entry.kind === 'change') {
    const changes = (item.changes || []).map((change) => `${change.kind || 'update'} ${change.path || ''}\n${change.diff || ''}`).join('\n\n')
    return `<pre class="activity-raw-code"><code>${escapeHtml(changes)}</code></pre>`
  }
  if (entry.kind === 'plan') {
    const rows = (item.plan || []).map((step) => `${step.status || 'pending'}  ${step.step || ''}`).join('\n')
    return `<pre class="activity-raw-text">${escapeHtml([item.explanation || '', rows].filter(Boolean).join('\n\n'))}</pre>`
  }
  return `<pre class="activity-raw-text">${escapeHtml(valueText(item))}</pre>`
}

function truncateForDisplay(value, max = 100) {
  const text = String(value || '').replace(/\s+/gu, ' ').trim()
  return text.length <= max ? text : `${text.slice(0, max - 1).trimEnd()}…`
}

function renderApprovals() {
  const approvals = state.model.approvals.map((approval) => {
    const params = approval.params || {}
    const command = Array.isArray(params.command) ? params.command.join(' ') : params.command || params.reason || [params.permission, ...(params.patterns || [])].filter(Boolean).join(' · ') || approval.method
    const permission = approval.method === 'item/permissions/requestApproval' || approval.method === 'opencode/permission'
    return `<section class="turn"><article class="approval-card" data-approval-id="${escapeHtml(String(approval.id))}">
      <strong>${permission ? t('{backend} requests additional permission', { backend: currentBackend().name }) : t('{backend} is waiting for approval', { backend: currentBackend().name })}</strong>
      <pre>${escapeHtml(command)}${params.cwd ? `\n${escapeHtml(params.cwd)}` : ''}</pre>
      <div class="approval-actions">
        <button class="subtle-button approval-decline" type="button">Decline</button>
        <button class="subtle-button approval-session" type="button">Allow for session</button>
        <button class="primary-button approval-accept" type="button">Allow once</button>
      </div>
    </article></section>`
  }).join('')
  return approvals + renderUserInteractions()
}

function bindApprovalButtons() {
  $$('.approval-card:not(.interaction-card)').forEach((card) => {
    if (card.dataset.approvalBound) return
    card.dataset.approvalBound = 'true'
    card.querySelector('.approval-decline').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'decline'))
    card.querySelector('.approval-session').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'acceptForSession'))
    card.querySelector('.approval-accept').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'accept'))
  })
  $$('.interaction-card').forEach((card) => {
    if (card.dataset.interactionBound) return
    card.dataset.interactionBound = 'true'
    card.querySelector('.interaction-submit')?.addEventListener('click', () => answerUserInteraction(card, 'accept'))
    card.querySelector('.interaction-decline')?.addEventListener('click', () => answerUserInteraction(card, 'decline'))
    card.querySelector('.interaction-cancel')?.addEventListener('click', () => answerUserInteraction(card, 'cancel'))
    card.querySelector('.interaction-open-url')?.addEventListener('click', () => {
      const url = card.dataset.url
      if (url) openBrowserUrl(url).catch(showError)
    })
  })
}

function renderUserInteractions() {
  return (state.model.interactions || []).map((interaction) => {
    const params = interaction.params || {}
    if (interaction.method === 'item/tool/requestUserInput') {
      const questions = (params.questions || []).map((question) => renderUserInputQuestion(question)).join('')
      return `<section class="turn"><article class="approval-card interaction-card" data-interaction-id="${escapeHtml(String(interaction.id))}" data-interaction-method="${escapeHtml(interaction.method)}">
        <strong>${t('Codex is waiting for your input')}</strong>
        <div class="interaction-fields">${questions}</div>
        <div class="approval-actions"><button class="subtle-button interaction-cancel" type="button">${t('Cancel')}</button><button class="primary-button interaction-submit" type="button">${t('Submit')}</button></div>
      </article></section>`
    }
    const mode = params.mode || 'form'
    const schema = params.requestedSchema || {}
    const fields = mode === 'url' ? '' : renderElicitationSchema(schema)
    return `<section class="turn"><article class="approval-card interaction-card" data-interaction-id="${escapeHtml(String(interaction.id))}" data-interaction-method="${escapeHtml(interaction.method)}" data-url="${escapeHtml(params.url || '')}">
      <strong>${escapeHtml(params.serverName || 'MCP')} ${t('is waiting for your input')}</strong>
      <p>${escapeHtml(params.message || '')}</p>
      <div class="interaction-fields">${fields}</div>
      <div class="approval-actions"><button class="subtle-button interaction-decline" type="button">${t('Decline')}</button>${mode === 'url' ? `<button class="subtle-button interaction-open-url" type="button">${t('Open link')}</button>` : ''}<button class="primary-button interaction-submit" type="button">${t(mode === 'url' ? 'Completed' : 'Submit')}</button></div>
    </article></section>`
  }).join('')
}

function renderUserInputQuestion(question = {}) {
  const name = `interaction-${String(question.id || randomId()).replace(/[^a-z0-9_-]/giu, '-')}`
  const options = Array.isArray(question.options) ? question.options : []
  const inputType = question.isSecret ? 'password' : 'text'
  const choices = options.map((option, index) => `<label class="interaction-option"><input type="radio" name="${escapeHtml(name)}" value="${escapeHtml(option.label || '')}" ${index === 0 ? 'checked' : ''}/><span><strong>${escapeHtml(option.label || '')}</strong><small>${escapeHtml(option.description || '')}</small></span></label>`).join('')
  const freeform = !options.length || question.isOther
    ? `<input class="interaction-freeform" data-question-id="${escapeHtml(question.id || '')}" type="${inputType}" placeholder="${escapeHtml(question.isOther ? t('Other…') : t('Enter a response…'))}" autocomplete="${question.isSecret ? 'off' : 'on'}" />`
    : ''
  return `<fieldset class="interaction-field" data-question-id="${escapeHtml(question.id || '')}" data-choice-name="${escapeHtml(name)}"><legend><span>${escapeHtml(question.header || '')}</span>${escapeHtml(question.question || '')}</legend>${choices}${freeform}</fieldset>`
}

function renderElicitationSchema(schema = {}) {
  const required = new Set(Array.isArray(schema.required) ? schema.required : [])
  return Object.entries(schema.properties || {}).map(([name, property]) => {
    const title = property.title || name
    const description = property.description ? `<small>${escapeHtml(property.description)}</small>` : ''
    const needed = required.has(name) ? ' required' : ''
    const values = property.enum || property.oneOf?.map((option) => option.const).filter((value) => value != null)
    const labels = property.enumNames || property.oneOf?.map((option) => option.title || option.const)
    if (Array.isArray(values)) {
      return `<label class="interaction-schema-field"><span>${escapeHtml(title)}</span><select data-field-name="${escapeHtml(name)}"${needed}>${values.map((value, index) => `<option value="${escapeHtml(String(value))}">${escapeHtml(String(labels?.[index] ?? value))}</option>`).join('')}</select>${description}</label>`
    }
    if (property.type === 'array') {
      const itemValues = property.items?.enum || property.items?.oneOf?.map((option) => option.const).filter((value) => value != null) || []
      const itemLabels = property.items?.enumNames || property.items?.oneOf?.map((option) => option.title || option.const)
      return `<label class="interaction-schema-field"><span>${escapeHtml(title)}</span><select data-field-name="${escapeHtml(name)}" data-field-array multiple${needed}>${itemValues.map((value, index) => `<option value="${escapeHtml(String(value))}">${escapeHtml(String(itemLabels?.[index] ?? value))}</option>`).join('')}</select>${description}</label>`
    }
    if (property.type === 'boolean') {
      return `<label class="interaction-schema-field interaction-checkbox"><input data-field-name="${escapeHtml(name)}" type="checkbox" ${property.default ? 'checked' : ''}/><span>${escapeHtml(title)}</span>${description}</label>`
    }
    const type = property.type === 'number' || property.type === 'integer' ? 'number' : property.format === 'password' ? 'password' : ['date', 'email', 'url'].includes(property.format) ? property.format : 'text'
    const step = property.type === 'integer' ? ' step="1"' : property.type === 'number' ? ' step="any"' : ''
    return `<label class="interaction-schema-field"><span>${escapeHtml(title)}</span><input data-field-name="${escapeHtml(name)}" type="${type}" value="${escapeHtml(property.default ?? '')}"${step}${needed}/>${description}</label>`
  }).join('') || `<p>${t('This request does not require additional fields.')}</p>`
}

function answerUserInteraction(card, action) {
  const id = card.dataset.interactionId
  const interaction = (state.model.interactions || []).find((candidate) => String(candidate.id) === String(id))
  if (!interaction) return
  if (interaction.method === 'item/tool/requestUserInput') {
    if (action !== 'accept') {
      sendRaw({ id: interaction.id, error: { code: -32001, message: 'User cancelled input' } })
    } else {
      const answers = {}
      for (const field of card.querySelectorAll('.interaction-field')) {
        const questionId = field.dataset.questionId
        const selected = field.querySelector(`input[name="${CSS.escape(field.dataset.choiceName)}"]:checked`)?.value
        const freeform = field.querySelector('.interaction-freeform')?.value.trim()
        const values = [freeform || selected].filter(Boolean)
        if (!values.length) {
          toast(t('Please answer every question'), 'error')
          return
        }
        answers[questionId] = { answers: values }
      }
      sendRaw({ id: interaction.id, result: { answers } })
    }
  } else {
    if (action !== 'accept') sendRaw({ id: interaction.id, result: { action } })
    else {
      const content = {}
      for (const input of card.querySelectorAll('[data-field-name]')) {
        if (!input.checkValidity()) {
          input.reportValidity()
          return
        }
        content[input.dataset.fieldName] = input.dataset.fieldArray != null
          ? [...input.selectedOptions].map((option) => option.value)
          : input.type === 'checkbox' ? input.checked : input.type === 'number' ? Number(input.value) : input.value
      }
      sendRaw({ id: interaction.id, result: { action: 'accept', content } })
    }
  }
  resolveCodexInteraction(state.model, interaction.id)
  renderTranscript()
}

async function answerApproval(id, decision) {
  const approval = state.model.approvals.find((candidate) => String(candidate.id) === String(id))
  if (!approval) return
  if (state.backend === 'opencode') {
    try {
      const reply = decision === 'decline' ? 'reject' : decision === 'acceptForSession' ? 'always' : 'once'
      await openCodeFetch(withDirectory(`/permission/${encodeURIComponent(id)}/reply`, selectedThread()?.cwd), { method: 'POST', body: { reply } })
      resolveCodexApproval(state.model, approval.id)
      renderTranscript()
    } catch (error) { showError(error) }
    return
  }
  let result
  if (approval.method === 'item/permissions/requestApproval') {
    result = decision === 'decline'
      ? { permissions: {} }
      : { scope: decision === 'acceptForSession' ? 'session' : 'turn', permissions: approval.params?.permissions || {} }
  } else {
    result = { decision }
  }
  sendRaw({ id: approval.id, result })
  resolveCodexApproval(state.model, approval.id)
  renderTranscript()
}

function handleComposerInput() {
  const input = $('#composer-input')
  composerDrafts.update(selectedStateKey(), input.value)
  renderComposerState()
  if (shellCommandFromComposer(input.value) !== null) {
    hideComposerMenu()
    return
  }
  const trigger = composerTrigger(input.value, input.selectionStart)
  const routerOptions = threadRouter.targetSuggestions(input.value, input.selectionStart)
  if (routerOptions) {
    clearTimeout(composerSearchTimer)
    const target = threadRouter.currentCandidates().find(candidate => candidate.key === state.routerRuntime.selectedTarget)
    const generation = state.composerMenu.generation + 1
    state.composerMenu = { type: 'router', trigger: routerOptions.trigger, options: routerOptions.options.map(option => ({ ...option, kind: 'session' })), selected: 0, generation,
      fileMessage: target?.cwd ? t('Searching files through Codex App Server…') : t('Choose a target session to search its files') }
    renderComposerMenu()
    if (target?.cwd) composerSearchTimer = setTimeout(() => performRouterFileSearch(routerOptions.trigger, generation, target), 120)
    return
  }
  if (!trigger) {
    hideComposerMenu()
    return
  }
  if (trigger.type === 'slash') {
    clearTimeout(composerSearchTimer)
    state.composerMenu = {
      type: 'slash',
      trigger,
      options: matchingSlashCommands(trigger.query),
      selected: 0,
      generation: state.composerMenu.generation + 1,
    }
    renderComposerMenu()
    return
  }
  if (trigger.type === 'skill') {
    searchComposerSkills(trigger)
    return
  }
  searchComposerFiles(trigger)
}

function handleComposerKeydown(event) {
  if (event.isComposing) return
  if (event.key === 'Enter' && event.shiftKey && (event.ctrlKey || event.metaKey)) {
    event.preventDefault()
    handleContinueAction()
    return
  }
  const menuOpen = !$('#composer-menu').classList.contains('hidden')
  if (menuOpen && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
    event.preventDefault()
    const direction = event.key === 'ArrowDown' ? 1 : -1
    const count = state.composerMenu.options.length
    if (count) state.composerMenu.selected = (state.composerMenu.selected + direction + count) % count
    renderComposerMenu()
    return
  }
  if (menuOpen && (event.key === 'Enter' || event.key === 'Tab') && state.composerMenu.options.length) {
    event.preventDefault()
    selectComposerOption(state.composerMenu.selected)
    return
  }
  if (menuOpen && event.key === 'Escape') {
    event.preventDefault()
    hideComposerMenu()
    return
  }
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    $('#composer-form').requestSubmit()
  }
}

function handleComposerMenuClick(event) {
  const openFile = event.target.closest('[data-open-file-index]')
  if (openFile) {
    event.stopPropagation()
    const option = state.composerMenu.options[Number(openFile.dataset.openFileIndex)]
    if (option && reviewableFileKind(option)) openArtifact(option).catch(showError)
    else toast('This file type cannot be opened in the document reviewer', 'error')
    return
  }
  const option = event.target.closest('[data-composer-index]')
  if (option) selectComposerOption(Number(option.dataset.composerIndex))
}

function hideComposerMenu() {
  $('#router-choose-target')?.setAttribute('aria-expanded', 'false')
  clearTimeout(composerSearchTimer)
  state.composerMenu.generation += 1
  state.composerMenu.type = null
  state.composerMenu.options = []
  state.composerMenu.trigger = null
  $('#composer-menu').classList.add('hidden')
  $('#composer-input').removeAttribute('aria-activedescendant')
}

function renderComposerMenu(message = '') {
  const menu = $('#composer-menu')
  const picker = Boolean(state.composerMenu.trigger?.confirmedPicker)
  menu.classList.toggle('router-choice-popup', picker)
  if (picker) {
    const anchor = $('#router-choose-target').getBoundingClientRect()
    const parent = $('#composer-form').getBoundingClientRect()
    menu.style.left = `${Math.max(8, Math.min(anchor.left - parent.left, parent.width - 368))}px`
    menu.style.bottom = `${parent.bottom - anchor.top + 6}px`
  } else { menu.style.left = ''; menu.style.bottom = '' }
  const options = state.composerMenu.options
  menu.classList.remove('hidden')
  if (!options.length && state.composerMenu.type !== 'router') {
    const empty = state.composerMenu.type === 'file'
      ? 'No matching files'
      : state.composerMenu.type === 'skill'
        ? 'No matching skills or plugins'
        : state.composerMenu.type === 'router' ? 'No matching sessions' : 'No matching commands'
    menu.innerHTML = `<div class="composer-menu-empty">${escapeHtml(t(message || empty))}</div>`
    return
  }
  menu.innerHTML = options.map((option, index) => {
    const selected = index === state.composerMenu.selected
    const type = state.composerMenu.type === 'router' ? option.kind === 'file' ? 'file' : 'router' : state.composerMenu.type
    const title = type === 'router' ? option.title : type === 'slash' ? `/${option.name}` : type === 'skill' ? `$${option.name}` : fuzzyFileLabel(option)
    const detail = option.automatic ? '' : type === 'router' ? `${backendDescriptor(option.backend).name} · ${option.cwd} · ${option.responsibility || option.openingMessage || ''}` : type === 'slash'
      ? option.description
      : type === 'skill'
        ? `${option.kind === 'plugin' ? 'Plugin' : 'Skill'} · ${option.description || option.shortDescription || option.interface?.shortDescription || option.scope || ''}`
        : option.root
    const previewable = type === 'file' && Boolean(reviewableFileKind(option))
    const openAction = type === 'file'
      ? `<button class="composer-file-open" type="button" data-open-file-index="${index}" title="${t(previewable ? 'Open for review' : 'Only text files and common images can be previewed')}"${previewable ? '' : ' disabled aria-disabled="true"'}>${t('Open')}</button>`
      : ''
    const group = !picker && state.composerMenu.type === 'router' && (index === 0 || option.kind !== options[index - 1].kind)
      ? `<div class="composer-menu-empty" role="presentation">${t(option.kind === 'file' ? 'Files' : 'Session')}</div>` : ''
    return `${group}<div id="composer-option-${index}" class="composer-option${selected ? ' selected' : ''}" role="option" aria-selected="${selected}" data-composer-index="${index}"><strong>${escapeHtml(title)}</strong><small>${escapeHtml(detail || '')}</small>${openAction}</div>`
  }).join('')
  if (state.composerMenu.type === 'router') {
    if (!options.some(option => option.kind === 'session')) menu.insertAdjacentHTML('afterbegin', `<div class="composer-menu-empty">${t('Session')} · ${t('No matching sessions')}</div>`)
    if (state.composerMenu.fileMessage) menu.insertAdjacentHTML('beforeend', `<div class="composer-menu-empty">${t('Files')} · ${escapeHtml(state.composerMenu.fileMessage)}</div>`)
  }
  $('#composer-input').setAttribute('aria-activedescendant', `composer-option-${state.composerMenu.selected}`)
  menu.querySelector('.selected')?.scrollIntoView({ block: 'nearest' })
}

async function performRouterFileSearch(trigger, generation, target) {
  const current = () => state.composerMenu.type === 'router' && state.composerMenu.generation === generation && state.routerRuntime.selectedTarget === target.key
  try {
    const result = await rpc('fuzzyFileSearch', { query: trigger.query, roots: [target.cwd], cancellationToken: randomId() }, 15_000)
    if (!current()) return
    const files = (Array.isArray(result?.files) ? result.files : []).slice(0, 30)
      .map(file => ({ ...file, root: target.cwd, kind: 'file', sourceSessionKey: target.key }))
    state.composerMenu.options = [...state.composerMenu.options.filter(option => option.kind === 'session'), ...files]
    state.composerMenu.fileMessage = files.length ? '' : t('No matching files')
    renderComposerMenu()
  } catch (error) {
    if (!current()) return
    state.composerMenu.fileMessage = t('File search failed: {message}', { message: error.message })
    renderComposerMenu()
  }
}

function searchComposerFiles(trigger) {
  const thread = selectedThread()
  if (!thread?.cwd) {
    hideComposerMenu()
    return
  }
  const generation = state.composerMenu.generation + 1
  state.composerMenu = { type: 'file', trigger, options: [], selected: 0, generation }
  renderComposerMenu('Searching files through Codex App Server…')
  clearTimeout(composerSearchTimer)
  composerSearchTimer = setTimeout(() => performComposerFileSearch(trigger, generation, thread.cwd), 120)
}

async function performComposerFileSearch(trigger, generation, cwd) {
  try {
    const result = await rpc('fuzzyFileSearch', {
      query: trigger.query,
      roots: [cwd],
      cancellationToken: randomId(),
    }, 15_000)
    if (generation !== state.composerMenu.generation || state.composerMenu.type !== 'file') return
    state.composerMenu.options = Array.isArray(result?.files) ? result.files.slice(0, 30) : []
    state.composerMenu.selected = 0
    renderComposerMenu()
  } catch (error) {
    if (generation !== state.composerMenu.generation) return
    renderComposerMenu(t('File search failed: {message}', { message: error.message }))
  }
}

function searchComposerSkills(trigger) {
  const cwd = selectedThread()?.cwd || ''
  const generation = state.composerMenu.generation + 1
  state.composerMenu = { type: 'skill', trigger, options: [], selected: 0, generation }
  renderComposerMenu('Discovering skills through Codex App Server…')
  loadSkillCatalog(cwd, false, true).then((skills) => {
    if (generation !== state.composerMenu.generation || state.composerMenu.type !== 'skill') return
    state.composerMenu.options = matchingSkills(trigger.query, skills)
    state.composerMenu.selected = 0
    renderComposerMenu()
  }).catch((error) => {
    if (generation !== state.composerMenu.generation) return
    renderComposerMenu(t('Failed to load skills: {message}', { message: error.message }))
  })
}

async function loadSkillCatalog(cwd, forceReload = false, includePlugins = false) {
  const catalog = state.skillCatalog
  const backend = state.backend
  const matches = catalog.cwd === cwd && catalog.backend === backend && catalog.includePlugins === includePlugins
  if (!forceReload && matches && catalog.loaded) return catalog.skills
  if (!forceReload && matches && catalog.request) return catalog.request
  const skillRequest = rpc('skills/list', { cwds: cwd ? [cwd] : [], forceReload })
    .then((result) => (result?.data || []).flatMap((entry) => entry.skills || []).filter((skill) => skill.enabled))
  const pluginRequest = includePlugins && isCodexBackend(backend)
    ? rpc('plugin/list', { cwds: cwd ? [cwd] : [], forceRefetch: forceReload }).then(pluginReferences).catch(error => {
      // Older app-servers may not implement plugins; other failures stay visible.
      if (error.code !== -32601 && !/method not found|unknown method|unsupported method/iu.test(error.message || '')) {
        toast(t('Failed to load plugins: {message}', { message: error.message }))
      }
      return []
    }) : Promise.resolve([])
  const request = Promise.all([skillRequest, pluginRequest]).then(([skills, plugins]) => [...plugins, ...skills])
  state.skillCatalog = { cwd, backend, includePlugins, skills: [], request, loaded: false }
  try {
    const skills = await request
    if (state.skillCatalog.request === request) state.skillCatalog = { cwd, backend, includePlugins, skills, request: null, loaded: true }
    return skills
  } catch (error) {
    if (state.skillCatalog.request === request) state.skillCatalog = { cwd: null, skills: [], request: null, loaded: false }
    throw error
  }
}

function selectComposerOption(index) {
  const option = state.composerMenu.options[index]
  const trigger = state.composerMenu.trigger
  if (!option || !trigger) return
  const input = $('#composer-input')
  if (state.composerMenu.type === 'router' && option.kind !== 'file') {
    threadRouter.chooseTarget(option.key)
    if (trigger.confirmedPicker) {
      hideComposerMenu()
      input.focus()
      return
    }
    const prefix = input.value.slice(0, trigger.start)
    const reference = `${prefix && !/\s$/u.test(prefix) ? ' ' : ''}@${option.title} `
    setCurrentComposerValue(prefix + reference + input.value.slice(trigger.end))
    input.setSelectionRange(prefix.length + reference.length, prefix.length + reference.length)
    hideComposerMenu()
    input.focus()
    return
  }
  if (state.composerMenu.type === 'file' || (state.composerMenu.type === 'router' && option.kind === 'file')) {
    const replacement = replaceComposerTrigger(input.value, trigger, selectedFileReference(option))
    setCurrentComposerValue(replacement.value)
    input.setSelectionRange(replacement.cursor, replacement.cursor)
    if (state.backend === 'opencode' && state.selectedId) {
      const key = selectedStateKey()
      const files = state.pendingFiles[key] ||= []
      if (!files.some((file) => file.path === option.path)) files.push({ type: 'file', path: option.path, root: option.root })
    }
    hideComposerMenu()
    renderComposerState()
    input.focus()
    return
  }
  if (state.composerMenu.type === 'skill') {
    const replacement = replaceComposerTrigger(input.value, trigger, selectedSkillReference(option))
    setCurrentComposerValue(replacement.value)
    input.setSelectionRange(replacement.cursor, replacement.cursor)
    addPendingSkill(option)
    hideComposerMenu()
    renderComposerState()
    input.focus()
    return
  }
  const replacement = replaceComposerTrigger(input.value, trigger, '')
  setCurrentComposerValue(replacement.value)
  hideComposerMenu()
  executeSlashCommand(option.action).catch(showError)
}

function showCommandDialog(title, content) {
  $('#command-title').textContent = title
  const commandContent = $('#command-content')
  commandContent.onclick = null
  commandContent.innerHTML = content
  const dialog = $('#command-dialog')
  if (dialog.open) dialog.close()
  dialog.showModal()
}

function currentTurnOptions() {
  if (!state.selectedId) return {}
  const key = selectedStateKey()
  state.turnOptions[key] ||= defaultTurnOptions(state.backend)
  return state.turnOptions[key]
}

function composerModelContext() {
  const target = isRouterThread() && state.routerRuntime.selectedTarget
    ? sessionRefFromKey(state.routerRuntime.selectedTarget) : null
  const ref = target || { backend: state.backend, id: state.selectedId }
  const key = selectedStateKey(ref.id, ref.backend)
  state.turnOptions[key] ||= defaultTurnOptions(ref.backend)
  return { ref, key, options: state.turnOptions[key], thread: threadForRef(ref), descriptor: backendDescriptor(ref.backend) }
}

function configuredTurnOptions(options = currentTurnOptions()) {
  const result = { ...options }
  const profile = state.environmentProfile
  if (!isCodexBackend(state.backend) || !profile?.configured || profile.root !== selectedThread()?.cwd) return result
  const current = result.sandboxPolicy
  if (!current || current.type === 'workspaceWrite') {
    result.sandboxPolicy = {
      type: 'workspaceWrite',
      writableRoots: current?.writableRoots || [profile.root],
      networkAccess: profile.networkPolicy === 'enabled',
    }
  } else if (current.type === 'readOnly') {
    result.sandboxPolicy = { ...current, networkAccess: profile.networkPolicy === 'enabled' }
  }
  return result
}

async function openModelCommand() {
  const context = composerModelContext()
  showCommandDialog('Model', '<div class="command-empty">Loading models from App Server…</div>')
  const models = await loadBackendModels({ refresh: true, backend: context.ref.backend })
  const currentEffort = context.options.effort
  const backendDefault = models.find((model) => model.isDefault)
  const backendDefaultId = String(backendDefault?.model || backendDefault?.id || '')
  const defaultCard = `<div class="command-card"><strong>${t('Use the backend default model')}</strong><small>${backendDefaultId ? t('Save the current backend default, {model}, for this session', { model: backendDefaultId }) : t('The backend did not identify a default model')}</small><span></span><button class="subtle-button" type="button" data-model-default${backendDefaultId ? '' : ' disabled'}>${t('Use')}</button></div>`
  $('#command-content').innerHTML = `<div class="command-list">${defaultCard}${models.map((model) => {
    const efforts = model.supportedReasoningEfforts || []
    const selectedEffort = efforts.some((entry) => entry.reasoningEffort === currentEffort)
      ? currentEffort
      : model.defaultReasoningEffort
    const effortOptions = efforts.map((entry) => `<option value="${escapeHtml(entry.reasoningEffort)}"${entry.reasoningEffort === selectedEffort ? ' selected' : ''}>${escapeHtml(entry.reasoningEffort)}</option>`).join('')
    return `<div class="command-card"><strong>${escapeHtml(model.displayName || model.model || model.id)}</strong><small>${escapeHtml(model.model || model.id)}${model.isDefault ? t(' · default') : ''}</small>${effortOptions ? `<select aria-label="${t('Reasoning effort')}">${effortOptions}</select>` : '<span></span>'}<button class="subtle-button" type="button" data-model="${escapeHtml(model.model || model.id)}">${t('Use')}</button></div>`
  }).join('')}</div>`
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-model], [data-model-default]')
    if (!button) return
    const key = context.key
    let options = state.turnOptions[key]
    const useDefault = button.hasAttribute('data-model-default')
    const effort = useDefault ? '' : button.closest('.command-card')?.querySelector('select')?.value
    if (useDefault) {
      if (!backendDefaultId) return
      options = { model: backendDefaultId }
      if (backendDefault.defaultReasoningEffort) options.effort = backendDefault.defaultReasoningEffort
      state.turnOptions[key] = options
      persistSessionTurnOptions(key).catch(showError)
    } else {
      options.model = button.dataset.model
      if (effort) options.effort = effort
      else delete options.effort
      persistSessionTurnOptions(key).catch(showError)
    }
    $('#command-dialog').close()
    renderComposerState()
    toast(useDefault
      ? t('This session now uses the saved backend default model {model}', { model: options.model })
      : t('Selected model {model}{effort}', { model: options.model, effort: effort ? ` · ${effort}` : '' }))
  }
}

async function loadBackendModels({ refresh = false, backend = state.backend } = {}) {
  if (!refresh && state.backendModels[backend]?.length) return state.backendModels[backend]
  if (!refresh && state.backendModelLoads.has(backend)) return state.backendModelLoads.get(backend)
  const load = dispatchBackendRpc(backend, 'model/list', { limit: 100, includeHidden: false })
    .then((result) => {
      const models = Array.isArray(result?.data) ? result.data : []
      state.backendModels[backend] = models
      if (composerModelContext().ref.backend === backend) renderComposerState()
      return models
    })
    .finally(() => state.backendModelLoads.delete(backend))
  state.backendModelLoads.set(backend, load)
  return load
}

function openPermissionsCommand() {
  if (state.backend === 'opencode') {
    showCommandDialog('Permissions', '<div class="command-empty">OpenCode permissions are managed by project configuration and runtime approvals; requests can be allowed once, always allowed, or denied.</div>')
    return
  }
  const choices = [
    ['readOnly', 'Read only', 'Files are read only; Codex requests approval when an action is needed'],
    ['workspaceWrite', 'Workspace write', 'Allows changes in the current project; network is disabled by default'],
    ['dangerFullAccess', 'Full access', 'Disables sandbox restrictions; use only for trusted projects'],
  ]
  showCommandDialog('Permissions', `<div class="command-list">${choices.map(([id, title, detail]) => `<button class="command-card" type="button" data-permission="${id}"><strong>${title}</strong><small>${detail}</small><span>Select</span></button>`).join('')}</div>`)
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-permission]')
    if (!button) return
    const type = button.dataset.permission
    if (type === 'dangerFullAccess' && !confirm(t('Use full access for future turns?'))) return
    const options = currentTurnOptions()
    options.approvalPolicy = type === 'dangerFullAccess' ? 'never' : 'on-request'
    options.sandboxPolicy = type === 'workspaceWrite'
      ? { type, writableRoots: [selectedThread()?.cwd].filter(Boolean), networkAccess: false }
      : { type }
    $('#command-dialog').close()
    renderComposerState()
    toast('Permissions updated for future turns')
  }
}

function openStatusCommand() {
  const thread = selectedThread()
  const options = currentTurnOptions()
  const rows = [
    ['Thread', thread?.name || thread?.id || '—'],
    ['Status', statusLabel(state.model.status)],
    ['Directory', thread?.cwd || '—'],
    ['Model', options.model || thread?.model || t('{backend} default', { backend: currentBackend().name })],
    ['Reasoning effort', options.effort || t('{backend} default', { backend: currentBackend().name })],
    ['Approval policy', options.approvalPolicy || 'Inherit session'],
    ['Sandbox', options.sandboxPolicy?.type || 'Inherit session'],
    ['Token', state.model.usage ? valueText(state.model.usage) : 'No data'],
  ]
  showCommandDialog('Session status', `<div class="command-summary">${rows.map(([label, value]) => `<div class="detail-row"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`).join('')}</div>`)
}

async function compactCurrentThread() {
  if (state.model.activeTurnId) throw new Error('The current turn is still running. Finish or stop it before compacting.')
  await rpc('thread/compact/start', { threadId: state.selectedId })
  toast('Codex started compacting the conversation')
}

async function reviewCurrentChanges() {
  if (state.model.activeTurnId) throw new Error('The current turn is still running. Finish or stop it before starting review.')
  beginTranscriptFollowingLatest()
  const result = await rpc('review/start', { threadId: state.selectedId, target: { type: 'uncommittedChanges' }, delivery: 'inline' })
  if (result?.turn) {
    applyCodexNotification(state.model, { method: 'turn/started', params: { turn: result.turn } })
    renderTranscript()
    renderComposerState()
  }
}

function openDiffCommand() {
  showCommandDialog('Current changes', state.model.diff
    ? `<pre class="command-pre">${escapeHtml(state.model.diff)}</pre>`
    : '<div class="command-empty">The current turn has no diff to display.</div>')
}

async function openMcpCommand() {
  showCommandDialog('MCP Server', '<div class="command-empty">Loading MCP status from App Server…</div>')
  const result = await rpc('mcpServerStatus/list', { limit: 100 })
  const servers = Array.isArray(result?.data) ? result.data : []
  $('#command-content').innerHTML = servers.length
    ? `<div class="command-list">${servers.map((server) => `<div class="command-card"><strong>${escapeHtml(server.name)}</strong><small>${t('{tools} tools · {resources} resources', { tools: Object.keys(server.tools || {}).length, resources: server.resources?.length || 0 })}</small><span>${escapeHtml(valueText(server.authStatus || 'unknown'))}</span></div>`).join('')}</div>`
    : '<div class="command-empty">No MCP servers are configured.</div>'
}

async function openSkillsCommand() {
  const cwd = selectedThread()?.cwd
  showCommandDialog('Skills', '<div class="command-empty">Discovering skills through App Server…</div>')
  const skills = await loadSkillCatalog(cwd || '')
  $('#command-content').innerHTML = skills.length
    ? `<div class="command-list">${skills.map((skill) => `<button class="command-card" type="button" data-skill-name="${escapeHtml(skill.name)}" data-skill-path="${escapeHtml(skill.path)}"><strong>$${escapeHtml(skill.name)}</strong><small>${escapeHtml(skill.description || skill.shortDescription || '')}</small><span>Reference</span></button>`).join('')}</div>`
    : '<div class="command-empty">No enabled skills are available in this directory.</div>'
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-skill-name]')
    if (!button || !state.selectedId) return
    addPendingSkill({ name: button.dataset.skillName, path: button.dataset.skillPath })
    const input = $('#composer-input')
    setCurrentComposerValue(`${input.value}${input.value && !input.value.endsWith(' ') ? ' ' : ''}$${button.dataset.skillName} `)
    $('#command-dialog').close()
    renderComposerState()
    input.focus()
  }
}

function addPendingSkill(skill) {
  const reference = composerReferenceInput(skill || {})
  if (!state.selectedId || !reference) return
  const key = selectedStateKey()
  const skillsForThread = state.pendingSkills[key] ||= []
  if (!skillsForThread.some((candidate) => candidate.path === skill.path)) {
    skillsForThread.push(reference)
  }
}

function latestAgentResponseText() {
  const items = state.model.turns.flatMap((turn) => turn.items || []).reverse()
  const message = items.find((item) => (item.type === 'agentMessage' || item.type === 'plan') && item.text)
  if (!message) return ''
  return message.type === 'agentMessage' ? sessionMapVisibleText(message.text) : message.text
}

async function copyLatestAgentResponse() {
  const text = latestAgentResponseText()
  const message = text.trim()
  if (!message) throw new Error(t('This session has no {backend} response to copy.', { backend: currentBackend().name }))
  await navigator.clipboard.writeText(text)
  toast(t('Copied the latest {backend} response', { backend: currentBackend().name }))
}

async function executeSlashCommand(action) {
  if (!state.selectedId && !['new'].includes(action)) throw new Error('Select a Codex session first.')
  const actions = {
    model: openModelCommand,
    permissions: openPermissionsCommand,
    status: openStatusCommand,
    compact: compactCurrentThread,
    review: reviewCurrentChanges,
    diff: openDiffCommand,
    skills: openSkillsCommand,
    mcp: openMcpCommand,
    rename: openRenameThreadDialog,
    fork: forkSelectedThread,
    new: openNewThreadDialog,
    copy: copyLatestAgentResponse,
    archive: archiveSelectedThread,
    delete: deleteSelectedThread,
  }
  const handler = actions[action]
  if (!handler) throw new Error(t('Command is not supported yet: /{action}', { action }))
  await handler()
}

function renderComposerState() {
  const active = Boolean(state.model.activeTurnId) || state.model.status === 'running'
  const awaitingTurnIdentity = active && !state.model.activeTurnId
  const context = composerModelContext()
  const options = context.options
  const descriptor = context.descriptor
  const display = resolveModelDisplay({
    overrideModel: options.model,
    overrideEffort: options.effort,
    sessionModel: context.thread?.model,
    models: state.backendModels[context.ref.backend] || [],
    fallback: `${descriptor.name} default`,
  })
  $('#composer-model-backend').textContent = descriptor.tag
  $('#composer-model-name').textContent = display.label
  $('#composer-model').title = `Current model and effort: ${display.label}`
  $('#composer-model').setAttribute('aria-label', `Current model and effort: ${display.label}`)
  const shellCommand = shellCommandFromComposer($('#composer-input').value)
  const shellMode = shellCommand !== null
  const queue = state.messageQueues[selectedStateKey()] || []
  const queueAvailable = active && !shellMode && !isRouterThread()
  const key = selectedStateKey()
  const hasComposerContent = composerHasPendingContent(key)
  const draftingContinuation = state.continuationDraftLoads.has(key)
  const continueAvailable = !active && !shellMode && !isRouterThread()
  $('#composer-form').classList.toggle('shell-mode', shellMode)
  renderComposerImages()
  $('#interrupt-turn').classList.toggle('hidden', !active)
  $('#interrupt-turn').disabled = !state.ready || awaitingTurnIdentity
  $('#queue-message').classList.toggle('hidden', !queueAvailable)
  $('#queue-message').textContent = queue.length >= state.queueDepth ? `${t('Queue')} (${queue.length}/${state.queueDepth})` : t('Queue')
  $('#continue-thread').classList.toggle('hidden', !continueAvailable)
  $('#continue-thread').textContent = draftingContinuation ? t('Drafting…') : t('Continue')
  $('#continue-thread').title = state.continueBehavior === 'quickSend'
    ? t('Immediately send a random continue prompt · Ctrl/Cmd+Shift+Enter')
    : state.continueBehavior === 'ollamaDraft'
      ? t('Draft the next message with local Ollama · Ctrl/Cmd+Shift+Enter')
      : t('Draft the next message with the current session model · Ctrl/Cmd+Shift+Enter')
  $('#continue-thread').disabled = !state.ready || !state.selectedId || active || hasComposerContent || Boolean(queue.length) || draftingContinuation
  $('#archive-thread').disabled = active || state.backend === 'opencode'
  $('#delete-thread').disabled = active
  $('#send-message').textContent = shellMode ? t('Run') : isRouterThread() ? t('Route') : active && isCodexBackend(state.backend) ? 'Steer' : 'Send'
  $('#send-message').classList.toggle('hidden', active && state.backend === 'opencode')
  $('#send-message').disabled = !state.ready || !state.selectedId || awaitingTurnIdentity || (active && (state.backend === 'opencode' || isRouterThread())) || (shellMode && (active || !shellCommand))
  $('#composer-add-image').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode')
  renderMessageQueue()
  if (queue.length && !active && !state.pausedMessageQueues.has(selectedStateKey()) && !state.runningMessageQueues.has(selectedStateKey())) {
    const ref = { backend: state.backend, id: state.selectedId }
    queueMicrotask(() => runNextQueuedMessage(ref).catch((error) => console.error('Queued turn failed', error)))
  }
  reviewNotes.renderComposerContext()
  threadRouter.renderComposerTarget()
  renderComposerTools()
}

function handleContinueAction(...args) {
  return composerActions.handleContinueAction(...args)
}

function quickSendContinueMessage(...args) {
  return composerActions.quickSendContinueMessage(...args)
}

function composerHasPendingContent(...args) {
  return composerActions.composerHasPendingContent(...args)
}

async function draftContinueMessage(...args) {
  return composerActions.draftContinueMessage(...args)
}

async function draftContinueWithOllama(...args) {
  return composerActions.draftContinueWithOllama(...args)
}

async function draftContinueWithSessionModel(...args) {
  return composerActions.draftContinueWithSessionModel(...args)
}

function ensureContinuationBackend(...args) {
  return composerActions.ensureContinuationBackend(...args)
}

function renderMessageQueue() {
  const key = selectedStateKey()
  const queue = state.messageQueues[key] || []
  const panel = $('#composer-message-queue')
  panel.classList.toggle('hidden', !queue.length)
  if (!queue.length) {
    $('#composer-queue-items').replaceChildren()
    return
  }
  const paused = state.pausedMessageQueues.has(key)
  const running = state.runningMessageQueues.has(key)
  const error = state.messageQueueErrors.get(key)
  $('#composer-queue-status').textContent = error
    ? `${t('Paused')} · ${error}`
    : running ? t('Sending…') : paused ? t('Paused') : state.model.activeTurnId ? t('Waiting for current turn') : t('Ready')
  $('#resume-message-queue').classList.toggle('hidden', !paused || Boolean(state.model.activeTurnId) || running)
  $('#composer-queue-items').innerHTML = queue.map((message, index) => `<div class="composer-queue-item" data-queue-id="${escapeHtml(message.id)}">
    <span class="composer-queue-index">${index + 1}</span>
    <span class="composer-queue-text" title="${escapeHtml(message.text)}">${escapeHtml(message.text || t('Message with attachments'))}</span>
    <span class="composer-queue-actions"><button type="button" data-queue-edit="${escapeHtml(message.id)}"${running ? ' disabled' : ''}>${t('Edit')}</button><button type="button" data-queue-delete="${escapeHtml(message.id)}"${running ? ' disabled' : ''}>${t('Delete')}</button></span>
  </div>`).join('')
}

async function queueComposerMessage(...args) {
  return composerActions.queueComposerMessage(...args)
}

function handleMessageQueueClick(event) {
  if (state.runningMessageQueues.has(selectedStateKey())) return
  const edit = event.target.closest('[data-queue-edit]')
  if (edit) openQueuedMessageEditor(edit.dataset.queueEdit)
  const remove = event.target.closest('[data-queue-delete]')
  if (remove) deleteQueuedMessage(remove.dataset.queueDelete).catch(showError)
}

function openQueuedMessageEditor(...args) {
  return composerActions.openQueuedMessageEditor(...args)
}

function closeQueuedMessageEditor(...args) {
  return composerActions.closeQueuedMessageEditor(...args)
}

async function saveEditedQueuedMessage(...args) {
  return composerActions.saveEditedQueuedMessage(...args)
}

async function deleteQueuedMessage(...args) {
  return composerActions.deleteQueuedMessage(...args)
}

function resumeSelectedMessageQueue(...args) {
  return composerActions.resumeSelectedMessageQueue(...args)
}

function renderComposerImages() {
  const container = $('#composer-images')
  const images = state.pendingImages[selectedStateKey()] || []
  const signature = `${selectedStateKey()}|${images.map((image) => image.id).join('|')}`
  if (container.dataset.signature === signature) return
  container.dataset.signature = signature
  container.classList.toggle('hidden', !images.length)
  container.innerHTML = images.map((image) => `<figure class="composer-image" title="${escapeHtml(`${image.name} · ${formatImageSize(image.size)}`)}">
    <img src="${escapeHtml(image.url)}" alt="${escapeHtml(image.name)}" />
    <button type="button" data-remove-composer-image="${escapeHtml(image.id)}" title="${t('Remove image')}" aria-label="${t('Remove image')}">×</button>
    <small>${escapeHtml(image.name)}</small>
  </figure>`).join('')
}

async function addComposerImages(fileList) {
  if (!state.selectedId) return
  const files = [...(fileList || [])].filter((file) =>
    file?.type?.startsWith('image/') || !file?.type || /\.(?:png|jpe?g|webp|gif)$/iu.test(String(file?.name || '')),
  )
  if (!files.length) return
  const key = selectedStateKey()
  const current = state.pendingImages[key] ||= []
  const available = MAX_COMPOSER_IMAGES - current.length
  if (available <= 0) throw new Error(t('You can attach up to {count} images.', { count: MAX_COMPOSER_IMAGES }))
  if (files.length > available) toast(t('Only the first {count} images were attached.', { count: available }), 'warning')
  const prepared = []
  let preparedBytes = current.reduce((total, image) => total + Number(image.size || 0), 0)
  for (const file of files.slice(0, available)) {
    try {
      const image = await prepareComposerImage(file)
      if (preparedBytes + image.size > MAX_COMPOSER_IMAGE_TOTAL_BYTES) {
        throw new Error('Image attachments must be 20 MiB or smaller in total.')
      }
      prepared.push(image)
      preparedBytes += image.size
    } catch (error) {
      throw new Error(t(error.message))
    }
  }
  if (selectedStateKey() !== key) return
  const destination = state.pendingImages[key] ||= []
  const remaining = Math.max(0, MAX_COMPOSER_IMAGES - destination.length)
  if (prepared.length > remaining) toast(t('Only the first {count} images were attached.', { count: remaining }), 'warning')
  let destinationBytes = destination.reduce((total, image) => total + Number(image.size || 0), 0)
  const accepted = prepared.slice(0, remaining).filter((image) => {
    if (destinationBytes + image.size > MAX_COMPOSER_IMAGE_TOTAL_BYTES) return false
    destinationBytes += image.size
    return true
  })
  if (accepted.length < Math.min(prepared.length, remaining)) throw new Error(t('Image attachments must be 20 MiB or smaller in total.'))
  destination.push(...accepted)
  renderComposerState()
}

function removeComposerImage(event) {
  const button = event.target.closest('[data-remove-composer-image]')
  if (!button) return
  const key = selectedStateKey()
  state.pendingImages[key] = (state.pendingImages[key] || []).filter((image) => image.id !== button.dataset.removeComposerImage)
  renderComposerState()
}

function handleComposerImagePaste(event) {
  const files = [...(event.clipboardData?.files || [])].filter((file) => file.type?.startsWith('image/'))
  if (files.length) addComposerImages(files).catch(showError)
}

function handleComposerImageDrag(event) {
  if (![...(event.dataTransfer?.types || [])].includes('Files')) return
  event.preventDefault()
  event.dataTransfer.dropEffect = 'copy'
  $('#composer-form').classList.add('image-drop-active')
}

function handleComposerImageDragLeave(event) {
  if (!event.currentTarget.contains(event.relatedTarget)) event.currentTarget.classList.remove('image-drop-active')
}

function handleComposerImageDrop(event) {
  event.preventDefault()
  event.currentTarget.classList.remove('image-drop-active')
  addComposerImages(event.dataTransfer?.files).catch(showError)
}

async function prepareComposerTurn(ref) {
  const preparedBySessionMap = await sessionMap.prepareTurn(ref)
  if (preparedBySessionMap) {
    sessionDispatch.markPrepared(ref)
    return
  }
  await sessionDispatch.prepareTurn(ref)
}

function sessionRefFromKey(key) {
  const separator = String(key || '').indexOf(':')
  return separator > 0 ? { backend: key.slice(0, separator), id: key.slice(separator + 1) } : null
}

function messageQueueModel(ref) {
  if (!ref) return null
  if (state.backend === ref.backend && state.selectedId === ref.id) return state.model
  return state.threadModels.get(threadCatalogKey(ref.backend, ref.id))?.model || null
}

function sessionTurnOptions(ref) {
  if (!ref) return {}
  return copySessionTurnOptions(state.turnOptions, selectedStateKey(ref.id, ref.backend), defaultTurnOptions(ref.backend))
}

function queuedTurnOptions(ref) {
  const options = sessionTurnOptions(ref)
  return selectedStateKey(ref.id, ref.backend) === selectedStateKey()
    ? configuredTurnOptions(options)
    : options
}

function pauseMessageQueue(ref, reason = '') {
  return submissionController.pauseMessageQueue(ref, reason)
}

function handleQueuedTurnCompletion(ref, status) {
  return submissionController.handleQueuedTurnCompletion(ref, status)
}

async function runNextQueuedMessage(ref) {
  return submissionController.runNextQueuedMessage(ref)
}

async function sendComposer(event) {
  return submissionController.sendComposer(event)
}

function isRouterThread(threadId = state.selectedId, backend = state.backend) {
  return threadRouter.isThread(threadId, backend)
}

async function ensureSessionModel(ref) {
  return backgroundSessions.ensureSessionModel(ref)
}

function mergeThreadIntoCatalog(backend, incoming) {
  return sessionOperations.mergeThreadIntoCatalog(backend, incoming)
}

function activateStartedThread(backend, thread, { operation = 'new' } = {}) {
  return sessionOperations.activateStartedThread(backend, thread, { operation })
}

async function interruptTurn(...args) {
  return composerActions.interruptTurn(...args)
}

function openNewThreadDialog() {
  $('#new-thread-error').classList.add('hidden')
  $('#new-thread-backend').value = state.backend
  updateNewThreadCapabilities()
  $('#new-thread-dialog').showModal()
  setTimeout(() => ($('#new-thread-name').value ? $('#new-thread-name') : $('#new-thread-cwd')).focus(), 30)
}

function closeNewThreadDialog() { $('#new-thread-dialog').close() }

function updateNewThreadCapabilities() {
  const backend = $('#new-thread-backend').value
  const unsupported = backend === 'opencode'
  $('#new-thread-model').placeholder = unsupported ? t('Optional: provider/model') : t('Use the Codex default model')
  for (const id of ['new-thread-approval', 'new-thread-sandbox']) {
    const select = $(`#${id}`)
    const field = select.closest('.field')
    select.disabled = unsupported
    field.classList.toggle('capability-disabled', unsupported)
    field.querySelector('.backend-capability-note')?.classList.toggle('hidden', !unsupported)
  }
  loadNewThreadModels(backend).catch((error) => {
    $('#new-thread-model-help').textContent = `Unable to load ${backendDescriptor(backend).name} models: ${error.message}`
  })
}

function handleNewThreadBackendChange() {
  $('#new-thread-model').value = ''
  $('#new-thread-model-options').replaceChildren()
  updateNewThreadCapabilities()
}

async function loadNewThreadModels(backend) {
  $('#new-thread-model-help').textContent = `Loading the independent model catalog from ${backendDescriptor(backend).name}…`
  const result = await dispatchBackendRpc(backend, 'model/list', { limit: 100, includeHidden: false })
  const models = Array.isArray(result?.data) ? result.data : []
  state.backendModels[backend] = models
  if ($('#new-thread-backend').value !== backend) return
  $('#new-thread-model-options').innerHTML = models.map((model) => {
    const id = model.model || model.id
    return id ? `<option value="${escapeHtml(id)}">${escapeHtml(model.displayName || model.name || id)}</option>` : ''
  }).join('')
  const defaultModel = models.find((model) => model.isDefault)
  $('#new-thread-model-help').textContent = `${backendDescriptor(backend).name} provides ${models.length} models${defaultModel ? ` · Default: ${defaultModel.model || defaultModel.id}` : ''}`
}

function openRenameThreadDialog() {
  const thread = selectedThread()
  if (!thread) return
  $('#rename-thread-name').value = thread.name || threadTitle(thread)
  $('#rename-thread-error').classList.add('hidden')
  $('#rename-thread-dialog').showModal()
  setTimeout(() => { $('#rename-thread-name').focus(); $('#rename-thread-name').select() }, 30)
}

function closeRenameThreadDialog() { $('#rename-thread-dialog').close() }

async function renameSelectedThread(event) {
  return sessionOperations.renameSelectedThread(event)
}

async function createThread(event) {
  return sessionOperations.createThread(event)
}

async function forkThread(lastTurnId = null, trigger = null) {
  return sessionOperations.forkThread(lastTurnId, trigger)
}

async function forkSelectedThread() {
  await forkThread()
}

async function archiveSelectedThread() {
  return sessionOperations.archiveSelectedThread()
}

async function deleteSelectedThread() {
  return sessionOperations.deleteSelectedThread()
}

function closeWorkspacePeerRails() {
  activateRightWorkspace('workspace')
}

function activateRightWorkspace(tool) {
  state.activeRightWorkspace = tool
  if (tool !== 'browser' && !state.embeddedBrowserVisible) applyRightRailWidth()
  const rails = {
    map: 'session-map-rail',
    document: 'artifact-rail',
    comments: 'annotation-rail',
    favorites: 'favorites-rail',
    resources: 'resources-rail',
  }
  for (const [candidate, id] of Object.entries(rails)) {
    $(`#${id}`).classList.toggle('hidden', candidate !== tool)
  }
  if (tool !== 'workspace') workspaceTools.close()
  if (tool !== 'browser') {
    state.embeddedBrowserVisible = false
    renderBrowserMenuStatus()
    window.location.href = 'studio-action://hide-browser'
    setTimeout(applyRightRailWidth, 80)
  }
  closeActionMenus()
  reviewNotes.hideSelection()
  syncRightWorkspaceLaunchers()
}

function deactivateRightWorkspace(tool) {
  if (state.activeRightWorkspace === tool) state.activeRightWorkspace = null
  syncRightWorkspaceLaunchers()
}

function syncRightWorkspaceLaunchers() {
  $('#open-thread-comments')?.setAttribute('aria-pressed', String(!$('#annotation-rail').classList.contains('hidden')))
  $('#open-thread-favorites')?.setAttribute('aria-pressed', String(!$('#favorites-rail').classList.contains('hidden')))
  $('#open-thread-resources')?.setAttribute('aria-pressed', String(!$('#resources-rail').classList.contains('hidden')))
}

function openArtifact(...args) {
  return documentWorkspace.open(...args)
}

function renderArtifact() {
  return documentWorkspace.render()
}

function closeArtifactRail(options) {
  return documentWorkspace.close(options)
}

function setArtifactView(view) {
  return documentWorkspace.setView(view)
}

function reopenEpubComment(anchor) {
  return documentWorkspace.reopenEpubSource(anchor)
}

function jumpArtifactToLine(line, column) {
  return documentWorkspace.jumpToLine(line, column)
}

function waitFor(predicate, timeoutMs) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now()
    const check = () => {
      if (predicate()) resolve()
      else if (Date.now() - startedAt >= timeoutMs) reject(new Error('Timed out waiting for backend switch'))
      else setTimeout(check, 80)
    }
    check()
  })
}

async function loadPreferences(...args) {
  return settingsApplication.loadPreferences(...args)
}

function preferencesSnapshot() {
  return createPreferencesSnapshot(state)
}

function persistPreferences() {
  if (!preferencesReady) return Promise.resolve()
  return preferencesWriter.write('/studio/preferences', preferencesSnapshot())
}

function persistAnnotationState(key) {
  return sessionPersistence.annotations(key)
}

function persistOpeningMessageState(key) {
  return sessionPersistence.openingMessage(key)
}

function persistSessionTurnOptions(key) {
  return sessionPersistence.turnOptions(key)
}

function persistMessageQueue(key) {
  return sessionPersistence.messageQueue(key)
}

function deletePersistedSessionState(key) {
  return sessionPersistence.remove(key)
}

function persistSessionPin(key, pinned) {
  return sessionPersistence.pin(key, pinned)
}

function setPinnedSessionLocal(key, pinned) {
  if (!key) return false
  if (pinned) {
    if (state.pinnedSessions.has(key)) return false
    state.pinnedSessions = new Set([...state.pinnedSessions, key])
    return true
  }
  return state.pinnedSessions.delete(key)
}

async function toggleSelectedThreadPin() {
  const key = selectedStateKey()
  if (!key || isArchivedPreview()) return
  const pinned = !state.pinnedSessions.has(key)
  if (pinned && state.pinnedSessions.size >= 10) {
    toast(t('You can pin up to 10 sessions.'), 'error')
    return
  }
  const previousPins = new Set(state.pinnedSessions)
  setPinnedSessionLocal(key, pinned)
  renderThreadList()
  syncPinThreadAction()
  try {
    await persistSessionPin(key, pinned)
    toast(t(pinned ? 'Session pinned' : 'Session unpinned'))
  } catch (error) {
    state.pinnedSessions = previousPins
    renderThreadList()
    syncPinThreadAction()
    showError(error)
  }
}

function syncPinThreadAction() {
  const action = $('#pin-thread')
  if (!action) return
  const pinned = state.pinnedSessions.has(selectedStateKey())
  action.querySelector('span').textContent = t(pinned ? 'Unpin session' : 'Pin session')
  action.setAttribute('aria-pressed', String(pinned))
}

function normalizeSharedDocumentDirectories(...args) {
  return settingsApplication.normalizeSharedDocumentDirectories(...args)
}

function isAbsoluteDocumentDirectory(...args) {
  return settingsApplication.isAbsoluteDocumentDirectory(...args)
}

function schedulePreferencesPersist() {
  clearTimeout(preferencesPersistTimer)
  preferencesPersistTimer = setTimeout(() => {
    preferencesPersistTimer = null
    persistPreferences()
  }, 150)
}

let activeSettingsPane = 'general'

function openSettings() {
  populateSettingsForm()
  $('#settings-dialog').showModal()
}

function activateSettingsPane(requestedPane, { focus = false } = {}) {
  const availableButtons = $$('#settings-navigation [data-settings-pane]').filter((button) => !button.classList.contains('hidden'))
  const button = availableButtons.find((candidate) => candidate.dataset.settingsPane === requestedPane) || availableButtons[0]
  if (!button) return
  activeSettingsPane = button.dataset.settingsPane
  for (const candidate of $$('#settings-navigation [data-settings-pane]')) {
    const active = candidate === button
    candidate.classList.toggle('active', active)
    candidate.setAttribute('aria-selected', String(active))
    candidate.tabIndex = active ? 0 : -1
  }
  for (const pane of $$('[data-settings-pane-content]')) {
    const active = pane.dataset.settingsPaneContent === activeSettingsPane
    pane.classList.toggle('active', active)
    pane.classList.toggle('hidden', !active)
  }
  if (focus) button.focus()
}

function handleSettingsNavigationClick(event) {
  const button = event.target.closest('[data-settings-pane]')
  if (!button || button.classList.contains('hidden')) return
  activateSettingsPane(button.dataset.settingsPane)
}

function handleSettingsNavigationKeydown(event) {
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return
  const buttons = $$('#settings-navigation [data-settings-pane]').filter((button) => !button.classList.contains('hidden'))
  if (!buttons.length) return
  const currentIndex = Math.max(0, buttons.indexOf(document.activeElement))
  let nextIndex = currentIndex
  if (event.key === 'Home') nextIndex = 0
  else if (event.key === 'End') nextIndex = buttons.length - 1
  else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + buttons.length) % buttons.length
  else nextIndex = (currentIndex + 1) % buttons.length
  event.preventDefault()
  activateSettingsPane(buttons[nextIndex].dataset.settingsPane, { focus: true })
}

function populateSettingsForm() {
  $('#language-select').value = state.language
  $('#theme-select').value = state.theme
  $('#content-width').value = state.contentWidth
  $('#queue-depth').value = String(state.queueDepth)
  $('#continue-behavior').value = state.continueBehavior
  $('#shared-document-directories').value = state.sharedDocumentDirectories.join('\n')
  $('#ui-font-family').value = state.typography.uiFontFamily
  $('#ui-font-size').value = String(state.typography.uiFontSize)
  $('#ui-font-weight').value = String(state.typography.uiFontWeight)
  $('#content-font-family').value = state.typography.contentFontFamily
  $('#content-font-size').value = String(state.typography.contentFontSize)
  $('#content-font-weight').value = String(state.typography.contentFontWeight)
  $('#code-font-family').value = state.typography.codeFontFamily
  $('#code-font-size').value = String(state.typography.codeFontSize)
  $('#code-font-weight').value = String(state.typography.codeFontWeight)
  $('#high-contrast').checked = state.typography.highContrast
  $('#desktop-notifications').checked = state.desktopNotifications
  const windowsHost = state.hostPlatform === 'windows'
  $('#settings-backends-navigation').classList.toggle('hidden', !windowsHost)
  $('#wsl-settings').classList.toggle('hidden', !windowsHost)
  $('#wsl-distribution').value = state.wsl.distribution
  $('#wsl-user').value = state.wsl.user
  $('#wsl-codex-binary').value = state.wsl.codexBinary
  $('#wsl-opencode-binary').value = state.wsl.opencodeBinary
  $('#annotation-template').value = state.activeAnnotationPromptTemplate
  populateTranslationSettingsForm()
  $('#settings-error').classList.add('hidden')
  activateSettingsPane(activeSettingsPane)
  loadOllamaModels({ refresh: true }).then(() => {
    if ($('#settings-dialog').open) renderOllamaModelOptions()
  }).catch((error) => console.warn('Unable to load Ollama models', error))
  if (state.translation.engine !== 'ollama') {
    loadBackendModels().then(() => {
      if ($('#settings-dialog').open) renderBackendTranslationModelOptions()
    }).catch(() => {})
  }
}

function populateTranslationSettingsForm() {
  const backend = state.backend
  const descriptor = backendDescriptor(backend)
  $('#translation-engine').value = state.translation.engine
  $('#translation-settings-backend').textContent = t('Current backend: {backend}', { backend: descriptor.name })
  $('#translation-model').value = state.translation.models[backend] || ''
  $('#translation-effort').value = state.translation.efforts[backend] ?? (isCodexBackend(backend) ? 'low' : '')
  $('#translation-ollama-model').value = state.translation.ollamaModel
  renderBackendTranslationModelOptions()
  renderOllamaModelOptions()
  syncTranslationSettingsEngine()
}

function renderBackendTranslationModelOptions() {
  const backend = state.backend
  $('#translation-model-options').innerHTML = (state.backendModels[backend] || []).map((entry) => {
    const id = String(entry.model || entry.id || '')
    return id ? `<option value="${escapeHtml(id)}">${escapeHtml(entry.displayName || entry.name || id)}</option>` : ''
  }).join('')
}

function renderOllamaModelOptions() {
  const select = $('#translation-ollama-model')
  const selected = select.value || state.translation.ollamaModel || 'gemma3:4b'
  const names = [...new Set(state.ollamaModels.map((entry) => entry.name).filter(Boolean))]
  if (!names.includes(selected)) names.unshift(selected)
  select.innerHTML = names
    .map((name) => `<option value="${escapeHtml(name)}">${escapeHtml(name)}</option>`)
    .join('')
  select.value = selected
}

function syncTranslationSettingsEngine() {
  const ollama = $('#translation-engine').value === 'ollama'
  for (const field of $$('.translation-backend-setting')) field.classList.toggle('hidden', ollama)
}

function handleTranslationEngineChange() {
  syncTranslationSettingsEngine()
  if ($('#translation-engine').value === 'ollama') {
    loadOllamaModels().then(renderOllamaModelOptions).catch((error) => {
      $('#settings-error').textContent = error.message
      $('#settings-error').classList.remove('hidden')
    })
  } else {
    loadBackendModels().then(renderBackendTranslationModelOptions).catch((error) => {
      $('#settings-error').textContent = error.message
      $('#settings-error').classList.remove('hidden')
    })
  }
}

async function loadOllamaModels({ refresh = false } = {}) {
  if (!refresh && state.ollamaModels.length) return state.ollamaModels
  const response = await gatewayFetch('/studio/ollama/models', { cache: 'no-store' })
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) throw new Error(payload?.error?.message || `Ollama HTTP ${response.status}`)
  state.ollamaModels = Array.isArray(payload.models)
    ? payload.models.filter((entry) => typeof entry?.name === 'string' && entry.name)
    : []
  return state.ollamaModels
}

async function saveSettings(...args) {
  return settingsApplication.saveSettings(...args)
}

async function fetchEnvironmentProfile(root) {
  const response = await gatewayFetch(`/studio/environment?root=${encodeURIComponent(root)}`, { cache: 'no-store' })
  const result = await response.json().catch(() => null)
  if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
  return result
}

async function openEnvironmentDialog(root = selectedThread()?.cwd || '') {
  closeActionMenus()
  root = String(root || '').trim()
  if (!root) {
    toast(t('The current session has no project directory.'), 'error')
    return
  }
  environmentDialogRoot = root
  environmentDialogProfile = null
  environmentSecretRemovals.clear()
  const name = basename(root) || root
  $('#environment-project-name').textContent = name
  $('#environment-project-root').textContent = root
  $('#environment-project-monogram').textContent = [...name][0]?.toUpperCase() || 'P'
  $('#environment-profile-status').className = 'environment-profile-status default'
  $('#environment-profile-status').textContent = t('Loading')
  $('#environment-error').classList.add('hidden')
  setEnvironmentDialogLoading(true)
  const dialog = $('#environment-dialog')
  if (dialog.open) dialog.close()
  dialog.showModal()
  try {
    const profile = await fetchEnvironmentProfile(root)
    if (!dialog.open || environmentDialogRoot !== root) return
    environmentDialogProfile = profile
    populateEnvironmentForm(profile)
    setEnvironmentDialogLoading(false)
  } catch (error) {
    if (!dialog.open || environmentDialogRoot !== root) return
    $('#environment-error').textContent = error.message
    $('#environment-error').classList.remove('hidden')
    setEnvironmentDialogLoading(false, { failed: true })
  }
}

function closeEnvironmentDialog() {
  const dialog = $('#environment-dialog')
  if (dialog.open) dialog.close()
  environmentDialogRoot = ''
  environmentDialogProfile = null
  environmentSecretRemovals.clear()
}

function setEnvironmentDialogLoading(loading, { failed = false } = {}) {
  $('#environment-loading').classList.toggle('hidden', !loading)
  $('#environment-form-content').classList.toggle('is-loading', loading || failed)
  const disabled = loading || failed
  for (const id of ['environment-variables', 'environment-secrets', 'environment-allowed-hosts', 'environment-cache-variables']) $(`#${id}`).disabled = disabled
  $$('input[name="environment-network-policy"]').forEach((input) => { input.disabled = disabled })
  $('#save-environment').disabled = disabled
}

function populateEnvironmentForm(profile) {
  $('#environment-variables').value = formatEnvironmentLines(profile?.variables)
  $('#environment-secrets').value = ''
  const policy = profile?.networkPolicy === 'enabled' ? 'enabled' : 'restricted'
  $$('input[name="environment-network-policy"]').forEach((input) => { input.checked = input.value === policy })
  $('#environment-allowed-hosts').value = (profile?.allowedHosts || []).join(', ')
  $('#environment-cache-variables').value = formatEnvironmentLines(profile?.cacheVariables)
  const status = $('#environment-profile-status')
  status.className = `environment-profile-status${profile?.configured ? '' : ' default'}`
  status.textContent = t(profile?.configured ? 'Configured' : 'Default environment')
  renderEnvironmentSecretNames()
  $('#environment-advanced').open = policy === 'enabled'
    || Boolean(profile?.allowedHosts?.length)
    || Boolean(Object.keys(profile?.cacheVariables || {}).length)
  updateEnvironmentDraftSummary()
}

function renderEnvironmentSecretNames() {
  const names = Array.isArray(environmentDialogProfile?.secretNames) ? environmentDialogProfile.secretNames : []
  $('#environment-secret-names').innerHTML = names.length
    ? names.map((name) => `<button class="environment-secret-chip" type="button" data-secret-name="${escapeHtml(name)}" aria-pressed="${environmentSecretRemovals.has(name)}" title="${escapeHtml(t('Click to mark for removal; click again to undo'))}">${escapeHtml(name)}</button>`).join('')
    : `<span class="environment-secret-empty">${escapeHtml(t('No saved secrets'))}</span>`
}

function toggleEnvironmentSecretRemoval(event) {
  const chip = event.target.closest('[data-secret-name]')
  if (!chip) return
  const name = chip.dataset.secretName
  if (environmentSecretRemovals.has(name)) environmentSecretRemovals.delete(name)
  else environmentSecretRemovals.add(name)
  renderEnvironmentSecretNames()
  updateEnvironmentDraftSummary()
}

function environmentDraftNames(value) {
  return String(value || '').split(/\r?\n/gu).map((line) => line.trim()).flatMap((line) => {
    const match = line.match(/^([A-Za-z_][A-Za-z0-9_]{0,127})\s*=/u)
    return match ? [match[1]] : []
  })
}

function selectedEnvironmentPolicy() {
  return $('input[name="environment-network-policy"]:checked')?.value === 'enabled' ? 'enabled' : 'restricted'
}

function updateEnvironmentDraftSummary() {
  const variableNames = new Set(environmentDraftNames($('#environment-variables').value))
  const cacheNames = new Set(environmentDraftNames($('#environment-cache-variables').value))
  const secretNames = new Set((environmentDialogProfile?.secretNames || []).filter((name) => !environmentSecretRemovals.has(name)))
  environmentDraftNames($('#environment-secrets').value).forEach((name) => secretNames.add(name))
  $('#environment-variable-count').textContent = String(variableNames.size)
  $('#environment-secret-count').textContent = String(secretNames.size)
  const policy = selectedEnvironmentPolicy() === 'enabled' ? 'Enabled' : 'Restricted'
  $('#environment-advanced-summary').textContent = cacheNames.size
    ? `${policy} · ${cacheNames.size} ${t('cache entries')}`
    : policy
}

async function saveProjectEnvironment(event) {
  event.preventDefault()
  const button = $('#save-environment')
  button.disabled = true
  button.textContent = t('Saving…')
  $('#environment-error').classList.add('hidden')
  try {
    await saveEnvironmentProfile()
    closeEnvironmentDialog()
    toast(t('Project environment saved'))
  } catch (error) {
    $('#environment-error').textContent = error.message
    $('#environment-error').classList.remove('hidden')
  } finally {
    button.disabled = false
    button.textContent = t('Save environment')
  }
}

async function saveEnvironmentProfile() {
  const root = environmentDialogRoot
  if (!root || !environmentDialogProfile) return
  const payload = environmentSavePayload({
    root,
    configured: Boolean(environmentDialogProfile.configured),
    secrets: $('#environment-secrets').value,
    removeSecrets: [...environmentSecretRemovals],
    variables: $('#environment-variables').value,
    allowedHosts: $('#environment-allowed-hosts').value,
    cacheVariables: $('#environment-cache-variables').value,
    networkPolicy: selectedEnvironmentPolicy(),
  })
  if (!payload) return
  const response = await gatewayFetch('/studio/environment', {
    method: 'PUT', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  const result = await response.json().catch(() => null)
  if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
  environmentDialogProfile = result
  if (selectedThread()?.cwd === root) {
    state.environmentProfile = result
    state.environmentProfileSelectionRoot = root
    await applyEnvironmentToCodex(result.root, {
      force: true,
      profileRevision: result.revision,
    })
    renderProjectEnvironmentEntry()
  }
}

async function activateSelectedEnvironment({ apply = true } = {}) {
  const backend = state.backend
  const threadId = state.selectedId
  const root = selectedThread()?.cwd || ''
  if (!root) {
    state.environmentProfile = null
    state.environmentProfileSelectionRoot = ''
    renderProjectEnvironmentEntry()
    return null
  }
  const profile = await fetchEnvironmentProfile(root)
  if (state.backend !== backend || state.selectedId !== threadId || selectedThread()?.cwd !== root) return null
  state.environmentProfile = profile
  state.environmentProfileSelectionRoot = root
  renderProjectEnvironmentEntry()
  if (apply && profile?.configured) {
    await applyEnvironmentToCodex(profile.root, {
      backend,
      threadId,
      profileRevision: profile.revision,
    })
    sessionDispatch.markPrepared({ backend, id: threadId })
  }
  return profile
}

function renderProjectEnvironmentEntry() {
  const action = $('#project-environment-action')
  const thread = selectedThread()
  const root = thread?.cwd || ''
  if (!action) return
  action.disabled = !root
  action.title = t(root ? 'Configure variables, secrets, network, and cache for this project' : 'The current session has no project directory')
  const configured = Boolean(root
    && state.environmentProfile?.configured
    && state.environmentProfileSelectionRoot === root)
  $('#project-environment-indicator').classList.toggle('hidden', !configured)
}

async function applyEnvironmentToCodex(root, {
  backend = state.backend,
  threadId = state.selectedId,
  includeThread = false,
  profileRevision = '',
  force = false,
  excludeTurns = true,
  initialTurnsPage = null,
} = {}) {
  return environmentApplication({
    root, backend, threadId,
    generation: state.appServerGenerations[backend] ?? 'unknown',
    includeThread, profileRevision, force, excludeTurns, initialTurnsPage,
  })
}

function resetSettings(...args) {
  return settingsApplication.resetSettings(...args)
}

function renderLocalizedUI() {
  applySidebarState()
  applyBackendCopy()
  updateNewThreadCapabilities()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  reviewNotes.renderFavorites()
  if (state.artifact) renderArtifact()
}

function applyAppearance() {
  const root = document.documentElement
  root.dataset.hostPlatform = state.hostPlatform || 'unknown'
  root.dataset.theme = state.theme
  root.dataset.contentWidth = state.contentWidth
  root.dataset.highContrast = String(state.typography.highContrast)
  root.dataset.markdownMode = state.markdown.mode
  root.style.setProperty('--ui-font-family', state.typography.uiFontFamily)
  root.style.setProperty('--ui-font-size', `${state.typography.uiFontSize}px`)
  root.style.setProperty('--ui-font-weight', state.typography.uiFontWeight)
  root.style.setProperty('--ui-font-emphasis', Math.min(700, state.typography.uiFontWeight + 100))
  root.style.setProperty('--content-font-family', state.typography.contentFontFamily)
  root.style.setProperty('--content-font-size', `${state.typography.contentFontSize}px`)
  root.style.setProperty('--content-font-weight', state.typography.contentFontWeight)
  root.style.setProperty('--content-font-emphasis', Math.min(700, state.typography.contentFontWeight + 100))
  root.style.setProperty('--code-font-family', state.typography.codeFontFamily)
  root.style.setProperty('--code-font-size', `${state.typography.codeFontSize}px`)
  root.style.setProperty('--code-font-weight', state.typography.codeFontWeight)
  workspaceTools.refreshTypography()
  syncEmbeddedBrowserTypography()
  resetMermaidRendering()
}

function backendStatusView(backend) {
  const status = state.backendStates[backend] || { kind: 'idle', label: backendDescriptor(backend).name, caption: 'Connect on demand' }
  const info = state.backendInfos[backend]
  if (backend === state.backend) return status
  if (info?.error) return { kind: 'error', label: 'Unavailable', caption: info.error }
  if (info?.reachable) return { kind: 'online', label: 'Available', caption: 'Session events connect on demand' }
  return { kind: 'idle', label: 'Connect on demand', caption: 'No session from this backend has been selected yet' }
}

function openConnectionsDialog() {
  closeActionMenus()
  renderConnectionsDialog()
  $('#connections-dialog').showModal()
  refreshBackendInformation()
}

function renderConnectionsDialog() {
  const configuration = state.backendRegistry
  const configuredCount = BACKEND_IDS.filter((backend) => !['codex', 'opencode'].includes(backend)).length
  const configCard = configuration.configPath || configuration.configurationError
    ? `<section class="backend-config-card${configuration.configurationError ? ' error' : ''}">
      <div><strong>Local backend configuration</strong><small data-no-i18n>${escapeHtml(configuration.configPath || 'Unavailable')}</small></div>
      <span>${configuration.configurationError ? 'Invalid' : `${configuredCount} configured`}</span>
      ${configuration.configurationError ? `<p>${escapeHtml(configuration.configurationError)}</p>` : '<p>Restart Studio after editing this file.</p>'}
    </section>`
    : ''
  const cards = BACKEND_IDS.map((backend) => {
    const descriptor = backendDescriptor(backend)
    const status = backendStatusView(backend)
    const info = state.backendInfos[backend] || {}
    return `<section class="connection-card ${descriptor.kind}">
      <span class="connection-monogram">${descriptor.tag}</span>
      <span class="connection-copy"><strong>${descriptor.name}</strong><small>${escapeHtml(info.binary || descriptor.binary)} · ${escapeHtml(info.transport || descriptor.transport)}</small></span>
      <span class="connection-state ${escapeHtml(status.kind)}">${escapeHtml(t(status.label))}</span>
    </section>`
  }).join('')
  $('#connections-dialog-content').innerHTML = `${configCard}${cards}`
}

function openBackendDialog() {
  closeActionMenus()
  renderBackendDialog()
  $('#backend-dialog').showModal()
  refreshBackendInformation()
}

function renderBackendDialog() {
  const appInfo = Object.values(state.backendInfos).find((info) => info?.appName) || {}
  const backendSections = BACKEND_IDS.map((backend) => {
    const descriptor = backendDescriptor(backend)
    const info = state.backendInfos[backend] || {}
    const status = backendStatusView(backend)
    return `<section class="about-section">
      <header>${descriptor.name}</header>
      <div class="detail-row"><span>${t('Status')}</span><strong>${escapeHtml(t(status.label))}</strong></div>
      <div class="detail-row"><span>${t('Executable')}</span><strong>${escapeHtml(info.binary || descriptor.binary)}</strong></div>
      <div class="detail-row"><span>${t('Backend version')}</span><strong>${escapeHtml(info.backendVersion || '—')}</strong></div>
      <div class="detail-row"><span>${t('Protocol')}</span><strong>${escapeHtml(info.protocol || descriptor.protocol)}</strong></div>
      <div class="detail-row"><span>${t('Transport')}</span><strong>${escapeHtml(info.transport || descriptor.transport)}</strong></div>
      <div class="detail-row"><span>${t('Execution environment')}</span><strong>${escapeHtml(info.executionEnvironment === 'wsl' ? `WSL · ${info.wslDistribution || t('Default distribution')}` : t('Local'))}</strong></div>
    </section>`
  }).join('')
  $('#backend-dialog-content').innerHTML = `<section class="about-section">
    <header>${t('Application')}</header>
    <div class="detail-row"><span>${t('Application')}</span><strong>${escapeHtml(appInfo.appName || 'Codex Thread Studio')}</strong></div>
    <div class="detail-row"><span>${t('Version')}</span><strong>v${escapeHtml(appInfo.appVersion || 'unknown')}</strong></div>
    <div class="detail-row"><span>${t('Runtime mode')}</span><strong>${t(state.remoteClient ? 'Remote workspace' : 'Local workspace')}</strong></div>
  </section>${backendSections}`
}

function refreshBackendInformation() {
  Promise.all(BACKEND_IDS.map((backend) => loadBackendInfo(backend))).then(() => {
    if ($('#connections-dialog').open) renderConnectionsDialog()
    if ($('#backend-dialog').open) renderBackendDialog()
  }).catch((error) => console.warn('Unable to refresh backend information', error))
}

function setBackendState(kind, label, caption) {
  state.backendStates[state.backend] = { kind, label, caption }
  const connection = $('#thread-connection')
  if (connection) {
    connection.className = `thread-connection ${kind}`
    const copy = kind === 'online' ? 'Connected' : kind === 'checking' ? 'Connecting…' : 'Disconnected'
    $('#native-connection').textContent = t(copy)
  }
  if ($('#connections-dialog').open) renderConnectionsDialog()
  if ($('#backend-dialog').open) renderBackendDialog()
}

function setNativeError(message) {
  $('#native-error').classList.toggle('hidden', !message)
  $('#native-error-message').textContent = t(message || '')
}

function threadTitle(thread) {
  return compactSidebarText(thread?.name || thread?.preview || basename(thread?.cwd) || thread?.id || t('Codex session'))
}
function basename(path) { return String(path || '').split(/[\\/]/).filter(Boolean).at(-1) || '' }
function shortId(value) { const text = String(value || ''); return text.length > 12 ? `${text.slice(0, 8)}…` : text }
function threadSourceLabel(source) {
  if (typeof source === 'string') return source
  if (!source || typeof source !== 'object') return ''
  return source.type || source.kind || Object.keys(source)[0] || ''
}
function threadStatus(thread) { return thread?.status?.type || thread?.status || 'notLoaded' }
function statusLabel(status) {
  return t(({ active: 'Running', running: 'Running', inProgress: 'In progress', idle: 'Idle', notLoaded: 'Not loaded', completed: 'Done', interrupted: 'Stopped', failed: 'Failed', systemError: 'Error', declined: 'Declined' })[status] || status || 'Unknown')
}
function arrayText(value) {
  if (!Array.isArray(value)) return typeof value === 'string' ? value : ''
  return value.map((entry) => typeof entry === 'string' ? entry : entry?.text || valueText(entry)).filter(Boolean).join('\n')
}
function valueText(value) { return typeof value === 'string' ? value : JSON.stringify(value, null, 2) }
function randomId() { return globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}` }
function truncateUtf8(value, limit) {
  const text = String(value || '')
  const encoded = new TextEncoder().encode(text)
  if (encoded.length <= limit) return text
  return new TextDecoder().decode(encoded.slice(0, limit)).replace(/\uFFFD$/u, '')
}
function truncateCharacters(value, limit) { return [...String(value || '')].slice(0, limit).join('') }
function isTypingTarget(target) { return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement || target?.isContentEditable }
function escapeHtml(value) { return String(value ?? '').replace(/[&<>'"]/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' })[character]) }




function normalizeTypography(value) { return normalizeTypographyProfile(value, typographyDefaults) }

function migrateDefaultFontFamilies(value) {
  const typography = value && typeof value === 'object' ? { ...value } : {}
  if (legacyDefaultUiFontFamilies.has(typography.uiFontFamily)) typography.uiFontFamily = defaultUiFontFamily
  if (!typography.contentFontFamily && typography.uiFontFamily) typography.contentFontFamily = typography.uiFontFamily
  if (!typography.contentFontWeight && typography.uiFontWeight) typography.contentFontWeight = typography.uiFontWeight
  if (legacyDefaultUiFontFamilies.has(typography.contentFontFamily)) typography.contentFontFamily = defaultUiFontFamily
  return typography
}



function normalizeAnnotationDrafts(value) {
  return normalizeCommentDrafts(value, {
    idFactory: randomId,
    migrateSource: legacyCommentSource,
    registry: commentSources,
  })
}

function toast(message, kind = 'info') {
  const element = document.createElement('div')
  element.className = `toast ${kind}`
  element.textContent = t(message)
  $('#toast-region').appendChild(element)
  setTimeout(() => element.remove(), 3200)
}
function showError(error) { console.error(error); reportClientError(error); toast(error?.message || String(error), 'error') }
