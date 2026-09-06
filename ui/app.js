import {
  applyCodexNotification,
  beginOptimisticCodexTurn,
  createCodexViewModel,
  hydrateCodexThread,
  reconcileOptimisticCodexTurn,
  resolveCodexApproval,
  resolveCodexInteraction,
  rollbackOptimisticCodexTurn,
  textFromUserContent,
} from './codex-native.mjs'
import {
  applyOpenCodeEvent,
  collectOpenCodeMessageHistory,
  collectOpenCodeMessageTail,
  collectOpenCodeRootSessions,
  createOpenCodeLoopGuard,
  fetchOpenCodeDirectoryStatuses,
  mergeOpenCodeMessagePages,
  mergeOpenCodeThreadTail,
  normalizeOpenCodeSessions,
  openCodeCommandTurn,
  openCodeModelList,
  openCodeThreadFromHistory,
  normalizeOpenCodeStatus,
  replayOpenCodeEventsAfterHistory,
  selectOpenCodeStartedUserMessage,
  sliceOpenCodeMessageTail,
  splitOpenCodeModel,
} from './opencode-native.mjs'
import { resolveModelDisplay } from './model-display.mjs'
import { createTranscriptDom } from './transcript-dom.mjs'
import {
  completedQueueShouldAdvance,
  normalizeQueueDepth,
  normalizeStoredMessageQueues,
} from './message-queue.mjs'
import {
  catalogListParams,
  mergeCatalogMetadata,
  reconcileStartedThreadCatalog,
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
import {
  composerTrigger,
  createComposerDraftStore,
  fuzzyFileLabel,
  matchingSkills,
  matchingSlashCommands,
  randomContinuePrompt,
  replaceComposerTrigger,
  reviewableFileKind,
  selectedFileReference,
  selectedSkillReference,
  shellCommandFromComposer,
  transcriptUpdateKind,
} from './composer-tools.mjs'
import {
  MAX_COMPOSER_IMAGES,
  MAX_COMPOSER_IMAGE_TOTAL_BYTES,
  composerImageInputs,
  formatImageSize,
  openCodeImagePart,
  prepareComposerImage,
  userImagesFromContent,
} from './composer-images.mjs'
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
import {
  MERMAID_PREFERENCES_DEFAULTS,
  mermaidInitializeConfig,
  normalizeMermaidPreferences,
} from './mermaid-config.mjs'
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
  reasoningStage,
  shouldShowTurnPlaceholder,
} from './transcript-presentation.mjs'
import {
  isTurnForkable,
  openCodeForkBody,
  threadForkParams,
} from './thread-fork.mjs'
import {
  SELECTION_TRANSLATION_INSTRUCTIONS,
  SELECTION_TRANSLATION_SCHEMA,
  selectionTranslationInput,
  translationCacheKey,
  translationTurnState,
} from './selection-translation.mjs'
import {
  CONTINUATION_DRAFT_INSTRUCTIONS,
  CONTINUATION_DRAFT_SCHEMA,
  continuationDraftInput,
  continuationDraftTurnState,
} from './continuation-draft.mjs'
import { waitForUtilityResult } from './utility-task.mjs'

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
import {
  catalogActivityTimestamp,
  catalogCountsWithAttention,
  catalogTimestamp,
  compactSidebarText,
  filterCatalogEntries,
  groupCatalogEntries,
  isCatalogCacheFresh,
  isSessionDirectoryHidden,
  normalizeHiddenSessionDirectories,
  partitionPinnedCatalogEntries,
  syncCatalogSelection,
  threadCatalogKey,
} from './thread-catalog.mjs'
import {
  addLoadedThread,
  preserveCatalogActivity,
  restoreCatalogThreadActivity,
  updateCatalogThreadActivity,
} from './thread-workset.mjs'
import {
  catalogsWithSingleRouter,
  finalAgentText,
  isRouterSession,
  managedRouterThread,
  migrateLegacyResponsibilities,
  normalizeThreadRouter,
  recoverManagedRouterCatalog,
  sessionRefKey,
} from './thread-router.mjs'
import {
  createThreadRouterController,
  createThreadRouterRuntimeState,
  routerRuntimeKey,
} from './thread-router-controller.mjs'
import { SessionDispatchRegistry, startTurnWithPreparation } from './session-dispatch.mjs'
import { createCodexHistoryLoader } from './codex-history-loader.mjs'
import { coordinateHistoryLoad } from './history-load-coordinator.mjs'
import { createSerializedStateWriter } from './serialized-state-writer.mjs'
import { normalizeStoredTurnOptions, sessionModelPreferencePayload, copySessionTurnOptions } from './session-model-preferences.mjs'
import { cachedSession, storeCachedSession, validateCachedModel, unvalidateCachedModel, cachedModelThreadId, routeCodexNotification } from './session-model-cache.mjs'
import DOMPurify from './vendor/purify.es.mjs'
import { formatEnvironmentLines, parseEnvironmentLines, parseHosts } from './environment-profile.mjs'
import { createPerformanceMonitor, exposePerformanceMonitor } from './performance-monitor.mjs'
import { transcriptModelRevision } from './model-revision.mjs'
import {
  codexLifecycleEvent,
  claimLifecycleNotification,
  codexLifecycleStreamMessage,
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
let preferencesPersistTimer = null
let transcriptFrame = null
const dirtyStreamItems = new Map()
const turnLatencyTraces = new Map()
let composerSearchTimer = null
let editingQueuedMessage = null
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
let openCodeEventStream = null
let openCodeEventStreamReady = false
let openCodeSelectionStartedWithoutEventBarrier = false
const openCodeEventStreamWaiters = new Set()
let openCodeEventStreamOpenCount = 0
let openCodeLifecycleReconciliation = null
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
const startedThreadsAwaitingCatalog = new Map()
const startedThreadCatalogTimers = new Map()
const codexCatalogRecoveryStarted = new Set()
let inactiveCatalogRefreshScheduled = false
let codexLifecycleSocket = null
let codexLifecycleReconnectTimer = null
let codexLifecycleConnectionGeneration = 0
let codexLifecycleStreamReady = false
let codexLifecycleOpenCount = 0
const handledCodexTurnLifecycleEvents = new Map()
const recentCodexStatusLifecycleEvents = new Map()
const codexBackendsNeedingRestartRecovery = new Set()
const codexOffscreenHistoryRefreshes = new Map()
const codexOffscreenHistoryRefreshRetries = new Map()
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
  dispatch: sessionDispatch,
  backend: {
    dispatchRpc: dispatchBackendRpc,
    refreshCatalogs: refreshRouterCatalogs,
    loadBackendInfo,
    configuredTurnOptions,
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
    renderItem,
    selectThread,
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
    openWorkspaceTool: (tool) => workspaceTools.open(tool),
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
  bindUI()
  await loadBackendRegistry()
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
    if (!event.target.closest('#selection-popover, .content-menu-anchor')) reviewNotes.hideSelection()
    if (!event.target.closest('.menu-anchor')) closeActionMenus()
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

  const utilityName = `Studio translation ${randomId()}`
  state.hiddenUtilityThreadNames.add(`${backend}:${utilityName}`)
  let threadId = ''
  let translationTask = null
  try {
    const started = await rpc('thread/start', {
      cwd,
      ...(model ? { model } : {}),
      ...(isCodexBackend(backend) ? {
        ephemeral: true,
        approvalPolicy: 'never',
        sandbox: 'read-only',
        developerInstructions: SELECTION_TRANSLATION_INSTRUCTIONS,
      } : { name: utilityName }),
    }, 30_000)
    threadId = String(started?.thread?.id || '')
    if (!threadId) throw new Error(t('The current backend did not create a translation task'))
    markUtilityThreadHidden(backend, threadId)
    ensureTranslationBackend(backend, generation)
    if (isCodexBackend(backend)) {
      const model = createCodexViewModel()
      model.threadId = threadId
      translationTask = { backend, threadId, turnId: '', model }
      state.structuredUtilityTasks.set(sessionRefKey(backend, threadId), translationTask)
    }

    const turnStarted = await rpc('turn/start', {
      threadId,
      cwd,
      input: [{ type: 'text', text: selectionTranslationInput(text) }],
      ...(!isCodexBackend(backend) ? { developerInstructions: SELECTION_TRANSLATION_INSTRUCTIONS } : {}),
      outputSchema: SELECTION_TRANSLATION_SCHEMA,
      ...(model ? { model } : {}),
      ...(effort ? { effort } : {}),
    }, 150_000)
    if (isCodexBackend(backend) && turnStarted?.turn?.id) {
      const turnId = String(turnStarted.turn.id)
      state.hiddenCodexTurns.add(routerRuntimeKey(backend, turnId))
      translationTask.turnId ||= turnId
      if (!translationTask.model.turns.some((turn) => String(turn.id) === turnId)) {
        applyCodexNotification(translationTask.model, {
          method: 'turn/started',
          params: { threadId, turn: turnStarted.turn },
        })
      }
    }

    const translation = await waitForUtilityResult({
      ensureCurrent: () => ensureTranslationBackend(backend, generation),
      read: async () => translationTurnState(isCodexBackend(backend)
        ? translationTask?.model
        : (await rpc('thread/read', { threadId, includeTurns: true, cwd }, 30_000))?.thread),
      intervalMs: isCodexBackend(backend) ? 100 : 350,
      timeoutMs: 150_000, timeoutMessage: t('Translation timed out'), errorMessage: t,
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
  } finally {
    if (threadId) state.structuredUtilityTasks.delete(sessionRefKey(backend, threadId))
    if (threadId) {
      dispatchBackendRpc(backend, 'thread/delete', {
        threadId,
        ...(!isCodexBackend(backend) ? { cwd } : {}),
      }, 15_000).catch((error) => {
        console.warn('Unable to remove the hidden translation session', error)
      }).finally(() => {
        const threadKey = sessionRefKey(backend, threadId)
        state.hiddenUtilityThreads.delete(threadKey)
        state.hiddenCodexThreads.delete(threadKey)
        if (translationTask?.turnId) state.hiddenCodexTurns.delete(routerRuntimeKey(backend, translationTask.turnId))
        state.hiddenUtilityThreadNames.delete(`${backend}:${utilityName}`)
      })
    } else {
      state.hiddenUtilityThreadNames.delete(`${backend}:${utilityName}`)
    }
  }
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
  if (openCodeEventStreamReady) return Promise.resolve(true)
  return new Promise((resolve) => {
    let settled = false
    const finish = (ready) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      openCodeEventStreamWaiters.delete(finish)
      resolve(ready)
    }
    const timer = setTimeout(() => finish(false), timeoutMs)
    openCodeEventStreamWaiters.add(finish)
  })
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
  if (openCodeEventStream) return openCodeEventStream
  const events = gatewayEventSource('/opencode/global/event')
  openCodeEventStream = events
  events.onopen = () => {
    const needsReconciliation = openCodeEventStreamOpenCount > 0
      || openCodeSelectionStartedWithoutEventBarrier
    openCodeEventStreamOpenCount += 1
    openCodeEventStreamReady = true
    openCodeSelectionStartedWithoutEventBarrier = false
    for (const waiter of [...openCodeEventStreamWaiters]) waiter(true)
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
  }
  events.onmessage = (event) => {
    try { handleOpenCodeServerEvent(JSON.parse(event.data)) }
    catch (error) { console.error('Invalid OpenCode SSE event', error, event.data) }
  }
  events.onerror = () => {
    openCodeEventStreamReady = false
    renderOpenCodeConnectionState('checking', 'Reconnecting to OpenCode', 'SSE event stream')
  }
  return events
}

function scheduleOpenCodeLifecycleReconciliation() {
  if (openCodeLifecycleReconciliation) return openCodeLifecycleReconciliation
  openCodeLifecycleReconciliation = new Promise((resolve) => {
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

function connectCodexLifecycleStream() {
  clearTimeout(codexLifecycleReconnectTimer)
  codexLifecycleReconnectTimer = null
  if (codexLifecycleSocket) {
    codexLifecycleSocket.onclose = null
    codexLifecycleSocket.close()
  }
  codexLifecycleStreamReady = false
  codexLifecycleConnectionGeneration += 1
  const generation = codexLifecycleConnectionGeneration
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/codex-lifecycle`)
  codexLifecycleSocket = socket

  return new Promise((resolve) => {
    let settled = false
    const settle = (ready) => {
      if (settled) return
      settled = true
      clearTimeout(barrierTimer)
      resolve(ready)
    }
    // This is only an initialization ordering barrier. The connection stays
    // alive and can become authoritative after a slow local WebSocket upgrade.
    const barrierTimer = setTimeout(() => settle(false), 750)

    socket.onmessage = (event) => {
      if (generation !== codexLifecycleConnectionGeneration) return
      let envelope
      try { envelope = codexLifecycleStreamMessage(JSON.parse(event.data)) }
      catch (error) {
        console.error('Invalid cross-backend Codex lifecycle message', error, event.data)
        return
      }
      if (!envelope) return
      if (envelope.type === 'ready') {
        const reconnected = codexLifecycleOpenCount > 0
        codexLifecycleOpenCount += 1
        codexLifecycleStreamReady = true
        settle(true)
        reportSessionLifecycle('codex-lifecycle-stream', {
          state: reconnected ? 'reconnected' : 'connected',
          backendCount: envelope.backends.length,
        })
        if (reconnected) scheduleCodexLifecycleReconciliation()
        return
      }
      if (!isCodexBackend(envelope.backend)) return
      if (envelope.message.method === 'studio/appServer/status') {
        handleInactiveCodexAppServerStatus(envelope.backend, envelope.message)
      } else if (envelope.message.method === 'studio/appServer/lagged') {
        handleCodexLifecycleLag(envelope.backend, envelope.message)
      } else {
        handleCodexLifecycleNotification(envelope.backend, envelope.message)
      }
    }
    socket.onerror = () => {
      if (generation !== codexLifecycleConnectionGeneration) return
      codexLifecycleStreamReady = false
    }
    socket.onclose = () => {
      if (generation !== codexLifecycleConnectionGeneration) return
      codexLifecycleStreamReady = false
      codexLifecycleSocket = null
      settle(false)
      reportSessionLifecycle('codex-lifecycle-stream', { state: 'disconnected' })
      codexLifecycleReconnectTimer = setTimeout(() => {
        if (generation !== codexLifecycleConnectionGeneration) return
        connectCodexLifecycleStream()
      }, 1_800)
    }
  })
}

function invalidateCodexBackendModelValidation(backend) {
  for (const [key, cached] of state.threadModels) {
    if (key.startsWith(`${backend}:`)) cached.validatedAt = 0
  }
}

function handleInactiveCodexAppServerStatus(backend, message) {
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

function handleCodexLifecycleLag(backend, message) {
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
    BACKEND_IDS
      .filter((backend) => backend !== state.backend && isCodexBackend(backend))
      .map((backend) => reconcileCodexLifecycleBackend(backend).catch((error) => {
        console.debug(`Unable to reconcile reconnected ${backend} lifecycle state`, error)
      })),
  ), { delay: 250, timeout: 1_500 })
}

async function reconcileCodexLifecycleBackend(backend) {
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
  backend,
  threadId,
  sourceModel,
  { advanceQueue = false } = {},
) {
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
  let refresh
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
    const model = createCodexViewModel()
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

function connectBackend({ backendInfoReady = false } = {}) {
  if (state.backend === 'opencode') connectOpenCode({ backendInfoReady }).catch(showError)
  else connectAppServer()
}

async function switchBackend(backend, { selectedId } = {}) {
  if (!isSupportedBackend(backend) || backend === state.backend) return
  const previousBackend = state.backend
  captureTranscriptViewState()
  state.selectedByBackend[state.backend] = state.selectedId
  cleanupConnections()
  const transitionGeneration = state.socketGeneration
  const backgroundConnected = isCodexBackend(previousBackend)
    ? codexLifecycleStreamReady
    : previousBackend === 'opencode' && openCodeEventStreamReady
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
  clearTimeout(state.reconnectTimer)
  cleanupSocket()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  const descriptor = currentBackend()
  setBackendState('checking', `Starting ${descriptor.name}`, 'App Server · stdio')
  setNativeError(null)
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
  state.socket = socket

  socket.onmessage = (event) => {
    if (generation !== state.socketGeneration) return
    try { handleAppServerMessage(JSON.parse(event.data)) }
    catch (error) { console.error('Invalid App Server message', error, event.data) }
  }
  socket.onerror = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('error', `${descriptor.name} Disconnected`, 'WebSocket connection failed')
  }
  socket.onclose = () => {
    if (generation !== state.socketGeneration) return
    state.ready = false
    rejectPending(new Error(`${descriptor.name} App Server connection closed`))
    setBackendState('error', t('{backend} disconnected', { backend: descriptor.name }), 'Preparing to reconnect…')
    setNativeError(t('The connection to the local {backend} App Server closed.', { backend: descriptor.name }))
    state.reconnectTimer = setTimeout(connectAppServer, 1800)
  }
}

async function connectOpenCode({ backendInfoReady = false } = {}) {
  clearTimeout(state.reconnectTimer)
  cleanupConnections()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  setBackendState('checking', 'Starting OpenCode', 'Server · HTTP/SSE')
  setNativeError(null)
  if (!backendInfoReady) await loadBackendInfo()
  if (generation !== state.socketGeneration) return
  if (state.backendInfo?.reachable === false || state.backendInfo?.error) {
    const reason = state.backendInfo.error || 'OpenCode Server is not ready'
    setBackendState('error', 'OpenCode unavailable', reason)
    setNativeError(reason)
    return
  }
  state.ready = true
  loadBackendModels().catch((error) => console.debug('Unable to load OpenCode models', error))
  startOpenCodeEventStream()
  // Prefer establishing the event barrier before taking the history snapshot.
  // If the stream is slow to open, the late first onopen advances the epoch and
  // schedules a second authoritative read so the gap still cannot be hidden.
  const eventStreamReady = await waitForOpenCodeEventStream()
  if (generation !== state.socketGeneration || state.backend !== 'opencode') return
  if (!eventStreamReady) openCodeSelectionStartedWithoutEventBarrier = true
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
      message.params?.turn?.status || message.params?.status || 'completed',
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

function handleAppServerMessage(message) {
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
          message: error.message,
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
    const pending = state.pending.get(String(message.id))
    if (!pending) return
    state.pending.delete(String(message.id))
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(message.error.message || JSON.stringify(message.error)))
    else pending.resolve(message.result)
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
    if (thread) thread.name = message.params.threadName
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

async function resynchronizeSelectedThreadAfterLag({ backend, threadId, socketGeneration, skipped }) {
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

async function captureOffscreenInteraction(message) {
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
  const cached = eventThreadId && state.threadModels.get(threadCatalogKey('opencode', eventThreadId))
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

function rpc(method, params = {}, timeoutMs = 30_000) {
  if (state.backend === 'opencode') return openCodeRpc(method, params, timeoutMs)
  if (!state.ready || state.socket?.readyState !== WebSocket.OPEN) return Promise.reject(new Error(t('{backend} App Server is not ready', { backend: currentBackend().name })))
  const id = ++state.requestId
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      state.pending.delete(String(id))
      reject(new Error(t('{method} request timed out', { method })))
    }, timeoutMs)
    state.pending.set(String(id), { resolve, reject, timer, method })
    sendRaw({ id, method, params })
  })
}

async function dispatchBackendRpc(backend, method, params = {}, timeoutMs = 30_000) {
  if (backend === state.backend && state.ready) return rpc(method, params, timeoutMs)
  if (isCodexBackend(backend)) return codexBackgroundRpc(backend, method, params, timeoutMs)
  if (backend === 'opencode') {
    await ensureOpenCodeAvailable()
    return openCodeRpc(method, params, timeoutMs, { allowInactive: true })
  }
  throw new Error(`Unsupported session backend: ${backend}`)
}

function codexBackgroundRpc(backend, method, params = {}, timeoutMs = 30_000) {
  return new Promise((resolve, reject) => {
    const descriptor = backendDescriptor(backend)
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
    const id = -(Date.now() + Math.floor(Math.random() * 100_000))
    let requested = false
    let settled = false
    const finish = (error, value) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      socket.onclose = null
      socket.close()
      if (error) reject(error)
      else resolve(value)
    }
    const timer = setTimeout(() => finish(new Error(t('{method} request timed out', { method }))), timeoutMs)
    socket.onmessage = (event) => {
      let message
      try { message = JSON.parse(event.data) } catch { return }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'error') {
        finish(new Error(message.params?.message || 'Codex App Server is unavailable'))
      } else if (message.method === 'studio/appServer/status' && message.params?.state === 'ready' && !requested) {
        requested = true
        socket.send(JSON.stringify({ id, method, params }))
      } else if (message.id === id) {
        if (message.error) finish(new Error(message.error.message || JSON.stringify(message.error)))
        else finish(null, message.result)
      }
    }
    socket.onerror = () => finish(new Error(t('Unable to connect to the {backend} App Server', { backend: descriptor.name })))
    socket.onclose = () => finish(new Error(t('The {backend} App Server connection closed', { backend: descriptor.name })))
  })
}

async function ensureOpenCodeAvailable() {
  const response = await gatewayFetch('/studio/opencode', { cache: 'no-store' })
  if (!response.ok) {
    const info = await response.json().catch(() => ({}))
    throw new Error(info.error || `OpenCode Server HTTP ${response.status}`)
  }
}

async function openCodeFetch(path, { method = 'GET', body, timeoutMs = 30_000, allowInactive = false, includeHeaders = false } = {}) {
  if (!allowInactive && (state.backend !== 'opencode' || !state.ready)) throw new Error('OpenCode Server is not ready')
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await gatewayFetch(`/opencode${path}`, {
      method,
      headers: body == null ? {} : { 'Content-Type': 'application/json' },
      body: body == null ? undefined : JSON.stringify(body),
      signal: controller.signal,
      cache: 'no-store',
    })
    if (response.status === 204) return includeHeaders ? { value: null, headers: response.headers } : null
    const text = await response.text()
    let value = null
    try { value = text ? JSON.parse(text) : null } catch { value = text }
    if (!response.ok) throw new Error(value?.error?.message || value?.message || `${method} ${path} failed: HTTP ${response.status}`)
    return includeHeaders ? { value, headers: response.headers } : value
  } catch (error) {
    if (error.name === 'AbortError') throw new Error(t('{method} {path} request timed out', { method, path }))
    throw error
  } finally { clearTimeout(timer) }
}

async function fetchOpenCodeMessageHistory(threadId, directory, fetchOptions, { anchorTurnIds = [] } = {}) {
  const path = `/session/${encodeURIComponent(threadId)}/message`
  const key = String(threadId || '')
  const finishHistoryFetch = studioPerformance.start('opencode.history.fetch', {
    backend: 'opencode',
    threadKey: threadCatalogKey('opencode', threadId),
  })
  let pageCount = 0
  let historyMode = 'full'
  let latestSnapshot = { before: openCodeHistoryEventSequences.get(key) || 0, after: openCodeHistoryEventSequences.get(key) || 0 }
  const messageSnapshots = {}
  const fetchPage = async ({ limit, before }) => {
    pageCount += 1
    const sequenceBefore = openCodeHistoryEventSequences.get(key) || 0
    const query = new URLSearchParams({ limit: String(limit) })
    if (before) query.set('before', before)
    const page = await openCodeFetch(withDirectory(path, directory, query.toString()), {
      ...fetchOptions,
      includeHeaders: true,
    })
    const result = {
      messages: page.value,
      cursor: page.headers.get('x-next-cursor'),
    }
    const sequenceAfter = openCodeHistoryEventSequences.get(key) || 0
    for (const message of Array.isArray(result.messages) ? result.messages : []) {
      const messageId = String(message?.info?.id || '')
      if (!messageId) continue
      messageSnapshots[messageId] = {
        afterSequence: sequenceBefore === sequenceAfter ? sequenceAfter : sequenceBefore,
        ambiguousThroughSequence: sequenceAfter,
      }
    }
    if (!before) latestSnapshot = {
      before: sequenceBefore,
      after: sequenceAfter,
    }
    return result
  }
  try {
    const anchors = (Array.isArray(anchorTurnIds) ? anchorTurnIds : []).filter(Boolean)
    let history = anchors.length
      ? await collectOpenCodeMessageTail(fetchPage, anchors)
      : await collectOpenCodeMessageHistory(fetchPage)
    if (history.matched) historyMode = 'tail'
    else if (anchors.length && history.complete) historyMode = 'full-tail-scan'
    else if (anchors.length) {
      history = await collectOpenCodeMessageHistory(fetchPage)
      const recoveredTail = sliceOpenCodeMessageTail(history.messages, anchors)
      if (recoveredTail.matched) {
        history = { ...history, ...recoveredTail, complete: false }
        historyMode = 'tail-wide-fallback'
      } else {
        historyMode = 'full-fallback'
      }
    }
    for (let attempt = 0; latestSnapshot.before !== latestSnapshot.after && attempt < 2; attempt += 1) {
      const latest = await fetchPage({ limit: history.matched ? 80 : 500 })
      if (Array.isArray(latest.messages)) {
        history.messages = mergeOpenCodeMessagePages([history.messages, latest.messages])
        if (history.matched) {
          const anchorIndex = history.messages.findIndex((message) => message?.info?.role === 'user'
            && String(message.info.id || '') === String(history.anchorTurnId || ''))
          if (anchorIndex >= 0) history.messages = history.messages.slice(anchorIndex)
        }
      }
    }
    finishHistoryFetch({
      outcome: 'loaded',
      historyMode,
      pageCount,
      messageCount: Array.isArray(history.messages) ? history.messages.length : 0,
      complete: history.complete !== false,
    })
    return {
      ...history,
      historyMode,
      historyMessageSnapshots: messageSnapshots,
    }
  } catch (error) {
    finishHistoryFetch({ outcome: 'failed', pageCount })
    throw error
  }
}

async function fetchOpenCodeStatusSnapshot(threadId, directory, fetchOptions) {
  const key = String(threadId || '')
  let value = {}
  let snapshot = { before: -1, after: Number.POSITIVE_INFINITY }
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const before = openCodeHistoryEventSequences.get(key) || 0
    try {
      value = await openCodeFetch(withDirectory('/session/status', directory), fetchOptions)
    } catch {
      return { value: {}, statusEventSequence: -1 }
    }
    snapshot = { before, after: openCodeHistoryEventSequences.get(key) || 0 }
    if (snapshot.before === snapshot.after) break
  }
  return {
    value,
    statusEventSequence: snapshot.before === snapshot.after ? snapshot.after : snapshot.before,
  }
}

async function fetchOpenCodeCatalog(limit = 100, { allowInactive = false, includeStatuses = true } = {}) {
  if (allowInactive) {
    const response = await gatewayFetch('/studio/opencode', { cache: 'no-store' })
    if (!response.ok) {
      const info = await response.json().catch(() => ({}))
      throw new Error(info.error || `OpenCode Server HTTP ${response.status}`)
    }
  }
  const options = { timeoutMs: 30_000, allowInactive }
  const sessions = await collectOpenCodeRootSessions(({ limit: pageLimit, archived, roots, cursor }) => {
    const query = new URLSearchParams({
      limit: String(pageLimit),
      archived: String(archived),
      roots: String(roots),
    })
    if (cursor != null) query.set('cursor', String(cursor))
    return openCodeFetch(`/experimental/session?${query}`, options)
  }, limit)
  if (!includeStatuses) return normalizeOpenCodeSessions(sessions, {})
  const directories = [...new Set((sessions || []).map((session) => session.directory).filter((cwd) =>
    cwd && !isSessionDirectoryHidden(cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore),
  ))]
  const statuses = await fetchOpenCodeDirectoryStatuses(
    directories,
    (cwd) => openCodeFetch(withDirectory('/session/status', cwd), options),
  )
  return normalizeOpenCodeSessions(sessions, statuses)
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
  return new Promise((resolve, reject) => {
    const descriptor = backendDescriptor(backend)
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}${descriptor.socketPath}`)
    const id = -(Date.now() + Math.floor(Math.random() * 100_000))
    const timer = setTimeout(() => finish(new Error(t('{backend} request timed out', { backend: descriptor.name }))), timeoutMs)
    let requested = false
    let settled = false
    const finish = (error, value) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      socket.onclose = null
      socket.close()
      if (error) reject(error)
      else resolve(value)
    }
    socket.onmessage = (event) => {
      let message
      try { message = JSON.parse(event.data) } catch { return }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'error') {
        finish(new Error(message.params?.message || t('{backend} App Server is unavailable', { backend: descriptor.name })))
        return
      }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'ready' && !requested) {
        requested = true
        socket.send(JSON.stringify({ id, method, params }))
        return
      }
      if (message.id !== id) return
      if (message.error) finish(new Error(message.error.message || `${method} failed`))
      else finish(null, message.result)
    }
    socket.onerror = () => finish(new Error(t('Unable to connect to the {backend} App Server', { backend: descriptor.name })))
    socket.onclose = () => finish(new Error(t('The {backend} App Server connection closed', { backend: descriptor.name })))
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

function startedThreadEntries(backend) {
  const prefix = `${backend}:`
  return [...startedThreadsAwaitingCatalog.entries()]
    .filter(([key]) => key.startsWith(prefix))
    .map(([, entry]) => entry)
}

function rememberStartedThread(backend, thread, operation) {
  if (!thread?.id) throw new Error(t('The backend created a session without an ID.'))
  const key = threadCatalogKey(backend, thread.id)
  startedThreadsAwaitingCatalog.set(key, {
    backend,
    operation,
    thread: { ...thread, turns: undefined },
  })
  // A catalog request that began before thread/start cannot know about this
  // in-memory session. Invalidate that response before it can replace the
  // active catalog; later requests retain the provisional entry below.
  catalogRequestGenerations.set(backend, (catalogRequestGenerations.get(backend) || 0) + 1)
  mergeThreadIntoCatalog(backend, thread)
  reportSessionLifecycle('started', {
    backend,
    threadId: thread.id,
    operation,
    awaitingCatalog: true,
  })
}

function forgetStartedThread(backend, threadId, reason) {
  if (!threadId) return false
  const key = threadCatalogKey(backend, threadId)
  const entry = startedThreadsAwaitingCatalog.get(key)
  if (!entry) return false
  startedThreadsAwaitingCatalog.delete(key)
  clearTimeout(startedThreadCatalogTimers.get(key))
  startedThreadCatalogTimers.delete(key)
  reportSessionLifecycle(reason, {
    backend,
    threadId,
    operation: entry.operation,
    awaitingCatalog: false,
  })
  return true
}

function reconcileCatalogWithStartedThreads(backend, threads) {
  const entries = startedThreadEntries(backend)
  if (!entries.length) return Array.isArray(threads) ? threads : []
  const currentCatalog = state.threadsByBackend[backend] || []
  const startedThreads = entries.map((entry) => {
    const current = currentCatalog.find((thread) => thread.id === entry.thread.id)
    if (current) entry.thread = { ...entry.thread, ...current, turns: undefined }
    return entry.thread
  })
  const reconciled = reconcileStartedThreadCatalog(threads, startedThreads)
  for (const threadId of reconciled.retainedIds) {
    const entry = startedThreadsAwaitingCatalog.get(threadCatalogKey(backend, threadId))
    if (!entry || entry.catalogMissLogged) continue
    entry.catalogMissLogged = true
    reportSessionLifecycle('catalog-retained', {
      backend,
      threadId,
      operation: entry.operation,
      awaitingCatalog: true,
    })
  }
  for (const threadId of reconciled.confirmedIds) {
    forgetStartedThread(backend, threadId, 'catalog-confirmed')
  }
  return reconciled.threads
}

function scheduleStartedThreadCatalogConfirmation(backend, threadId, delay = 800) {
  const key = threadCatalogKey(backend, threadId)
  if (!startedThreadsAwaitingCatalog.has(key)) return
  clearTimeout(startedThreadCatalogTimers.get(key))
  const timer = setTimeout(() => {
    startedThreadCatalogTimers.delete(key)
    if (!startedThreadsAwaitingCatalog.has(key)) return
    refreshBackendCatalog(backend, { includeStatuses: false }).catch((error) => {
      console.debug(`Unable to confirm newly started ${backend} session in the catalog`, error)
    })
  }, delay)
  startedThreadCatalogTimers.set(key, timer)
}

function clearStartedThreadsForBackend(backend, reason) {
  for (const entry of startedThreadEntries(backend)) {
    forgetStartedThread(backend, entry.thread.id, reason)
  }
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

function directoryQuery(directory = selectedThread()?.cwd) {
  return directory ? `directory=${encodeURIComponent(directory)}` : ''
}

function withDirectory(path, directory, extra = '') {
  return `${path}?${[directoryQuery(directory), extra].filter(Boolean).join('&')}`
}

async function openCodeRpc(method, params = {}, timeoutMs = 30_000, { allowInactive = false } = {}) {
  const catalog = allowInactive ? state.threadsByBackend.opencode : state.threads
  const thread = catalog.find((candidate) => candidate.id === (params.threadId || state.selectedId))
  const directory = params.cwd || thread?.cwd || ''
  const fetchOptions = { timeoutMs, allowInactive }
  if (method === 'thread/list') {
    return { data: await fetchOpenCodeCatalog(Number(params.limit || 100), { allowInactive }) }
  }
  if (method === 'thread/unsubscribe') return {}
  if (method === 'thread/resume' || method === 'thread/read') {
    const session = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}`, directory), fetchOptions)
    const history = await fetchOpenCodeMessageHistory(params.threadId, session.directory, fetchOptions, {
      anchorTurnIds: params.historyAnchorTurnIds,
    })
    const statuses = await fetchOpenCodeStatusSnapshot(params.threadId, session.directory, fetchOptions)
    return {
      thread: openCodeThreadFromHistory(session, history.messages, statuses.value?.[session.id] || 'idle'),
      historyComplete: history.complete,
      historyMessageSnapshots: history.historyMessageSnapshots,
      historyStatusSequence: statuses.statusEventSequence,
      historyAnchorTurnId: history.anchorTurnId,
      historyMode: history.historyMode,
    }
  }
  if (method === 'thread/start') {
    const model = splitOpenCodeModel(params.model)
    const session = await openCodeFetch(withDirectory('/session', params.cwd), { method: 'POST', body: { ...(params.name ? { title: params.name } : {}), ...(model ? { model: { id: model.modelID, providerID: model.providerID } } : {}) }, timeoutMs, allowInactive })
    return { thread: normalizeOpenCodeSessions([session], {})[0] }
  }
  if (method === 'thread/name/set') {
    return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}`, directory), { method: 'PATCH', body: { title: params.name }, timeoutMs })
  }
  if (method === 'thread/fork') {
    const session = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/fork`, directory), { method: 'POST', body: openCodeForkBody(params), timeoutMs })
    return { thread: normalizeOpenCodeSessions([session], {})[0] }
  }
  if (method === 'thread/delete') return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}`, directory), { method: 'DELETE', timeoutMs })
  if (method === 'thread/archive') throw new Error('OpenCode does not provide a separate archive action. You can rename, fork, or delete the session.')
  if (method === 'turn/start') {
    const text = textFromUserContent(params.input)
    const model = splitOpenCodeModel(params.model)
    const skill = params.input?.find((item) => item?.type === 'skill')
    if (skill?.name) {
      if (params.input?.some((item) => item?.type === 'image' || item?.type === 'localImage')) {
        throw new Error(t('OpenCode cannot combine a skill command with image attachments. Remove the skill or images and try again.'))
      }
      const result = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/command`, directory), {
        method: 'POST',
        body: { command: skill.name, arguments: text, agent: 'build' },
        timeoutMs: Math.max(timeoutMs, 300_000),
      })
      const turn = openCodeCommandTurn(result, params.input)
      if (!turn) throw new Error(t('OpenCode completed the command but did not return its user message ID.'))
      return { turn }
    }
    const imageParts = (params.input || []).map(openCodeImagePart).filter(Boolean)
    const parts = [
      ...(text ? [{ type: 'text', text }] : []),
      ...(params.input || []).filter((item) => item?.type === 'file').map(openCodeFilePart),
      ...imageParts,
    ]
    const baselineIds = await openCodeUserMessageBaseline(params.threadId, directory, fetchOptions)
    await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/prompt_async`, directory), {
      method: 'POST',
      body: {
        parts,
        ...(model ? { model } : {}),
        ...(params.developerInstructions ? { system: params.developerInstructions } : {}),
        ...(params.outputSchema ? { format: { type: 'json_schema', schema: params.outputSchema, retryCount: 2 } } : {}),
      },
      timeoutMs,
      allowInactive,
    })
    // OpenCode owns message identity. Older servers compare their monotonic
    // IDs to decide whether a prompt loop is complete, so a client UUID can
    // keep the same user prompt running forever. Read back the authoritative
    // user ID for Router/optimistic correlation instead of supplying one.
    return openCodeStartedTurn(params.threadId, directory, params.input, baselineIds, {
      ...fetchOptions,
      // The server has already accepted the prompt. Keep resolving its
      // authoritative user ID even if the operator switches backends now.
      allowInactive: true,
    })
  }
  if (method === 'turn/steer') throw new Error('Messages cannot be added while OpenCode is running. Wait for completion or stop it first.')
  if (method === 'turn/interrupt') return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/abort`, directory), { method: 'POST', timeoutMs })
  if (method === 'thread/shellCommand') {
    const options = currentTurnOptions()
    const model = splitOpenCodeModel(options.model || thread?.model)
    if (!model) throw new Error('Choose a model with /model before running an OpenCode shell command.')
    return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/shell`, directory), { method: 'POST', body: { agent: 'build', model, command: params.command }, timeoutMs })
  }
  if (method === 'fuzzyFileSearch') {
    const query = encodeURIComponent(params.query || '')
    const files = await openCodeFetch(withDirectory('/find/file', params.roots?.[0] || directory, `query=${query}&type=file&dirs=false&limit=200`), { timeoutMs })
    return { files: (files || []).map((path) => ({ path, root: params.roots?.[0] || directory })) }
  }
  if (method === 'model/list') {
    const providers = await openCodeFetch(withDirectory('/config/providers', directory), { timeoutMs })
    return { data: openCodeModelList(providers) }
  }
  if (method === 'skills/list') {
    const commands = await openCodeFetch(withDirectory('/command', params.cwds?.[0] || directory), { timeoutMs })
    return { data: [{ skills: (commands || []).filter((command) => command.source === 'skill').map((command) => ({ name: command.name, path: command.name, description: command.description || '', enabled: true })) }] }
  }
  if (method === 'mcpServerStatus/list') {
    const servers = await openCodeFetch(withDirectory('/mcp', directory), { timeoutMs })
    return { data: Object.entries(servers || {}).map(([name, status]) => ({ name, status })) }
  }
  if (method === 'thread/compact/start' || method === 'review/start') {
    const command = method === 'review/start' ? 'review' : 'compact'
    return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/command`, directory), { method: 'POST', body: { command, arguments: '', agent: 'build' }, timeoutMs })
  }
  throw new Error(t('The OpenCode backend does not support {method} yet', { method }))
}

function openCodeFilePart(file) {
  const root = String(file.root || selectedThread()?.cwd || '').replace(/\/$/u, '')
  const path = String(file.path || '').replace(/^\.\//u, '')
  const absolute = path.startsWith('/') ? path : `${root}/${path}`
  const label = `@${path}`
  return {
    type: 'file',
    mime: 'text/plain',
    filename: path,
    url: `file://${encodeURI(absolute)}`,
    source: { type: 'file', path, text: { value: label, start: 0, end: label.length } },
  }
}

function openCodeTurnResult(userId, input) {
  return {
    turn: {
      id: userId,
      status: 'inProgress',
      items: [{ id: userId, type: 'userMessage', content: input }],
    },
  }
}

async function openCodeUserMessageBaseline(threadId, directory, fetchOptions) {
  const path = `/session/${encodeURIComponent(threadId)}/message`
  const messages = await openCodeFetch(withDirectory(path, directory, 'limit=20'), fetchOptions)
  return new Set((Array.isArray(messages) ? messages : [])
    .filter((message) => message?.info?.role === 'user' && message.info.id)
    .map((message) => String(message.info.id)))
}

async function openCodeStartedTurn(threadId, directory, input, baselineIds, fetchOptions) {
  const path = `/session/${encodeURIComponent(threadId)}/message`
  const expectedText = textFromUserContent(input).trim()
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const messages = await openCodeFetch(withDirectory(path, directory, 'limit=20'), fetchOptions)
    const message = selectOpenCodeStartedUserMessage(messages, { baselineIds, expectedText })
    const info = message?.info
    if (info?.id) return openCodeTurnResult(info.id, input)
    await new Promise((resolve) => setTimeout(resolve, 30 * (attempt + 1)))
  }
  throw new Error(t('OpenCode accepted the prompt but did not return its user message ID.'))
}

function sendRaw(message) {
  if (state.socket?.readyState !== WebSocket.OPEN) throw new Error('Codex App Server connection is not open')
  state.socket.send(JSON.stringify(message))
}

function rejectPending(error) {
  for (const pending of state.pending.values()) {
    clearTimeout(pending.timer)
    pending.reject(error)
  }
  state.pending.clear()
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
    await switchBackend(backend, { selectedId: id })
    await waitFor(() => state.backend === backend && state.ready, 15_000)
    const selectionLoad = backendSelectionLoads.get(backend)
    if (selectionLoad) await selectionLoad
    await waitFor(() => state.threads.some((thread) => thread.id === id), 15_000)
    // The ready handler's loadThreads() owns the initial selection. Waiting for
    // its cache avoids running a second selection/render/environment flow in
    // parallel with the same single-flight history request.
    await waitFor(() => state.backend === backend
      && state.selectedId === id
      && Boolean(freshThreadModel(backend, id)), 30_000)
    return
  }
  if (!force && state.selectedId === id) return
  sessionManagement.search.close({ clear: true })
  closeActionMenus()
  hideComposerMenu()
  resetStreamingPatches()
  captureTranscriptViewState()
  if (state.artifact?.threadKey !== sessionMapKey(state.backend, id)) closeArtifactRail({ restoreMap: false })
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
  if (fresh) {
    $('#native-connection').textContent = t('Restored from cache')
    await mapLoad
    await environmentLoad
    if (state.backend !== backend || state.selectedId !== id) return
    sessionMap.maybeBootstrap(key, state.model)
    return
  }
  if (cached) $('#native-connection').textContent = t('Checking for updates…')
  const environmentProfile = isCodexBackend(backend) ? await environmentLoad : null
  if (state.backend !== backend || state.selectedId !== id) return
  const environmentRoot = isCodexBackend(backend) && environmentProfile?.configured
    ? environmentProfile.root
    : ''
  await resumeThread(id, {
    environmentRoot,
    environmentRevision: environmentProfile?.revision || '',
  })
  await Promise.all([mapLoad, isCodexBackend(backend) ? null : environmentLoad])
  if (state.backend !== backend || state.selectedId !== id) return
  sessionMap.maybeBootstrap(key, state.model)
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
    const installedTail = backend === 'opencode' && result.historyAnchorTurnId
      ? mergeOpenCodeThreadTail(state.model, result.thread, result.historyAnchorTurnId)
      : false
    if (!installedTail) hydrateCodexThread(state.model, result.thread)
    if (backend === 'opencode' && !installedTail) hydrateOpenCodeModelMetadata(result.thread, state.model, result.historyComplete)
    if (historyEvents?.length) replayOpenCodeEventsAfterHistory(state.model, historyEvents, id, {
      messageSnapshots: result.historyMessageSnapshots,
      statusAfterSequence: result.historyStatusSequence,
      authoritativeStatus: result.historyStatusSequence >= 0 ? result.thread?.status : null,
    })
    markTranscriptHistoryReady({
      complete: installedTail ? state.model.historyComplete !== false : result.historyComplete !== false,
    })
    mergeThreadMetadata(result.thread)
    cacheThreadModel(backend, id, state.model, { historyEpoch })
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
    const installedTail = backend === 'opencode' && result.historyAnchorTurnId
      ? mergeOpenCodeThreadTail(state.model, result.thread, result.historyAnchorTurnId)
      : false
    if (!installedTail) hydrateCodexThread(state.model, result.thread)
    if (backend === 'opencode' && !installedTail) hydrateOpenCodeModelMetadata(result.thread, state.model, result.historyComplete)
    if (historyEvents?.length) replayOpenCodeEventsAfterHistory(state.model, historyEvents, threadId, {
      messageSnapshots: result.historyMessageSnapshots,
      statusAfterSequence: result.historyStatusSequence,
      authoritativeStatus: result.historyStatusSequence >= 0 ? result.thread?.status : null,
    })
    markTranscriptHistoryReady({
      complete: installedTail ? state.model.historyComplete !== false : result.historyComplete !== false,
    })
    mergeThreadMetadata(result.thread)
    cacheThreadModel(backend, threadId, state.model, { historyEpoch })
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
  model.messageTurns = thread?.messageTurns || {}
  model.messageRoles = thread?.messageRoles || {}
  model.messageItems = thread?.messageItems || {}
  model.messageErrors = thread?.messageErrors || {}
  model.historyComplete = historyComplete !== false
  model.status = thread?.status || model.status
  model.activeTurnId = model.status === 'running' ? model.turns.at(-1)?.id || null : null
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
  if (!turn?.id) return false
  const existing = model.turns.find((candidate) => String(candidate?.id || '') === String(turn.id))
  const terminal = ['completed', 'failed', 'cancelled', 'interrupted'].includes(existing?.status)
  if (turn.status === 'inProgress' && terminal) return false
  const method = turn.status === 'inProgress' ? 'turn/started' : 'turn/completed'
  return applyCodexNotification(model, { method, params: { threadId, turn } })
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
  sessionMap.render()
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
    }
  })
  const signature = `${presentationThreadKey()}\u0000${state.language}\u0000${items
    .map((item) => `${item.id}\u0001${item.label}\u0001${item.title}`).join('\u0000')}`
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
    return `<button class="turn-nav-item" type="button" data-turn-nav-id="${escapeHtml(item.id)}" aria-label="${escapeHtml(item.label)}"><span class="turn-nav-title">${escapeHtml(item.title)}</span><span class="turn-nav-indicator" aria-hidden="true"><i></i></span></button>`
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
  const key = presentationThreadKey()
  const transcript = $('#transcript')
  if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
  transcriptPresentationCache.setScrollState(key, null)
  transcriptScrollFollower.pause()
  transcriptPresentationCache.showTurn(key, state.model, button.dataset.turnNavId)
  let target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .find((turn) => turn.dataset.turnId === button.dataset.turnNavId)
  if (!target) {
    renderTranscript()
    target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
      .find((turn) => turn.dataset.turnId === button.dataset.turnNavId)
  }
  if (!target) {
    transcriptScrollFollower.reset()
    followTranscriptOutput()
    return
  }
  const top = transcript.scrollTop + target.getBoundingClientRect().top - transcript.getBoundingClientRect().top - 16
  setActiveTurnNavigator(button.dataset.turnNavId)
  transcript.scrollTop = Math.max(0, top)
  transcriptCaptureSuppressedKeys.delete(key)
  captureTranscriptViewState()
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
  if (block.type === 'user') return renderItem(block.item, turnId)
  if (block.type === 'assistant') return renderItem(block.item, turnId, { forkable: options.forkable })
  if (block.type === 'activity') return renderActivity(block, turnId, options)
  if (block.type === 'error') return `<div class="turn-error" role="alert"><span class="message-track-mark turn-error-mark" aria-hidden="true">${conversationTrackIcon('failed')}</span><div class="turn-error-content"><strong>${t('Execution failed')}</strong><span>${escapeHtml(block.message)}</span></div></div>`
  return ''
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

function activityPresentationForTurn(turnId) {
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
  const block = activityBlockForTurn(details.dataset.turnId, details.dataset.activityId)
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

function renderItem(item, turnId, { forkable = false } = {}) {
  const type = item?.type || 'unknown'
  const attrs = `data-turn-id="${escapeHtml(turnId || '')}" data-item-id="${escapeHtml(item?.id || '')}"`
  if (type === 'userMessage') {
    const text = textFromUserContent(item.content)
    const images = renderUserMessageImages(item.content)
    return `<div class="message user" ${attrs}><span class="message-track-mark user-track-mark" aria-hidden="true">${conversationTrackIcon('question')}</span><div class="message-content">${images}${text ? `<div>${escapeHtml(text)}</div>` : (!images ? escapeHtml(t('(non-text input)')) : '')}</div></div>`
  }
  if (type === 'agentMessage' || type === 'plan') {
    const favorite = reviewNotes.favoriteForSource(state.backend, state.selectedId, turnId, item.id)
    const favoriteLabel = favorite ? 'Favorited; click to view' : 'Favorite this response'
    const forkAction = forkable
      ? `<button class="message-fork-button" type="button" data-fork-turn="${escapeHtml(turnId || '')}" title="${t('Fork from here')}" aria-label="${t('Fork from here')}"><svg viewBox="0 0 18 18" aria-hidden="true"><circle cx="4.25" cy="4" r="1.65"></circle><circle cx="4.25" cy="14" r="1.65"></circle><circle cx="13.75" cy="9" r="1.65"></circle><path d="M4.25 5.65v6.7M5.9 4h2.15a4.05 4.05 0 0 1 4.05 4.05V9"></path></svg><b>${t('Fork from here')}</b></button>`
      : ''
    return `<div class="message agent${favorite ? ' favorited' : ''}" ${attrs}>
      <span class="message-track-mark agent-track-mark" aria-hidden="true">${conversationTrackIcon('response')}</span>
      <div class="message-content"><div class="markdown-body">${renderMarkdown(type === 'agentMessage' ? sessionMapVisibleText(item.text) : item.text || '')}</div>
      <div class="message-actions"><button class="message-copy-button" type="button" data-copy-message="${escapeHtml(item.id || '')}" title="${t('Copy content')}" aria-label="${t('Copy content')}"><svg viewBox="0 0 18 18" aria-hidden="true"><rect x="2.75" y="2.75" width="8.5" height="10" rx="1.5"></rect><rect x="6.75" y="5.25" width="8.5" height="10" rx="1.5"></rect></svg><b>${t('Copy')}</b></button><button class="message-favorite-button${favorite ? ' active' : ''}" type="button" data-favorite-message="${escapeHtml(item.id || '')}" title="${favoriteLabel}" aria-label="${favoriteLabel}" aria-pressed="${Boolean(favorite)}"><svg viewBox="0 0 18 18" aria-hidden="true"><path d="m9 2.8 2.02 4.09 4.51.66-3.27 3.18.77 4.5L9 13.11l-4.03 2.12.77-4.5-3.27-3.18 4.51-.66Z"></path></svg><b>${favorite ? 'Favorited' : 'Favorites'}</b></button>${forkAction}</div></div>
    </div>`
  }
  if (type === 'reasoning') {
    const summary = arrayText(item.summary) || arrayText(item.content) || t('{backend} is reasoning…', { backend: currentBackend().name })
    return `<details class="reasoning" ${attrs} open><summary>Reasoning summary</summary><div class="markdown-body compact-markdown">${renderMarkdown(summary)}</div></details>`
  }
  if (type === 'commandExecution') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    return `<article class="item-card" ${attrs}><header><span>${t('Command')} · ${escapeHtml(command)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${item.aggregatedOutput ? `<pre>${escapeHtml(item.aggregatedOutput)}</pre>` : ''}</article>`
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
  const resourceLink = event.target.closest('.markdown-body a[data-resource-target]')
  if (resourceLink) {
    const target = resourceLink.dataset.resourceTarget || ''
    if (!target || target.startsWith('#')) return
    event.preventDefault()
    if (/^https?:\/\//iu.test(target)) {
      await openBrowserUrl(target)
      return
    }
    const file = resolveMarkdownFileLink(target)
    if (!file) return
    const openLinkedArtifact = () => openArtifact(
      { root: selectedThread()?.cwd, path: file.path },
      { returnTool: state.activeRightWorkspace === 'resources' ? 'resources' : '' },
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
    openActivityLog(activityLog.dataset.activityLog)
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
      state.backend,
      state.selectedId,
      element.dataset.turnId,
      element.dataset.itemId,
    )
    if (existing) await reviewNotes.openFavoriteDetail(existing.id)
    else reviewNotes.openFavoriteForMessage(element.dataset.turnId, element.dataset.itemId)
    return
  }
  const copyMessageButton = event.target.closest('[data-copy-message]')
  if (copyMessageButton) {
    const element = copyMessageButton.closest('[data-turn-id][data-item-id]')
    const item = element && modelItem(element.dataset.turnId, element.dataset.itemId)
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

function openActivityLog(turnId) {
  const entries = presentationActivityEntries(activityPresentationForTurn(turnId))
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
  clearTimeout(composerSearchTimer)
  state.composerMenu.type = null
  state.composerMenu.options = []
  state.composerMenu.trigger = null
  $('#composer-menu').classList.add('hidden')
  $('#composer-input').removeAttribute('aria-activedescendant')
}

function renderComposerMenu(message = '') {
  const menu = $('#composer-menu')
  const options = state.composerMenu.options
  menu.classList.remove('hidden')
  if (!options.length) {
    const empty = state.composerMenu.type === 'file'
      ? 'No matching files'
      : state.composerMenu.type === 'skill'
        ? 'No matching skills'
        : 'No matching commands'
    menu.innerHTML = `<div class="composer-menu-empty">${escapeHtml(t(message || empty))}</div>`
    return
  }
  menu.innerHTML = options.map((option, index) => {
    const selected = index === state.composerMenu.selected
    const type = state.composerMenu.type
    const title = type === 'slash' ? `/${option.name}` : type === 'skill' ? `$${option.name}` : fuzzyFileLabel(option)
    const detail = type === 'slash'
      ? option.description
      : type === 'skill'
        ? option.description || option.shortDescription || option.interface?.shortDescription || option.scope
        : option.root
    const previewable = type === 'file' && Boolean(reviewableFileKind(option))
    const openAction = type === 'file'
      ? `<button class="composer-file-open" type="button" data-open-file-index="${index}" title="${t(previewable ? 'Open for review' : 'Only text files and common images can be previewed')}"${previewable ? '' : ' disabled aria-disabled="true"'}>${t('Open')}</button>`
      : ''
    return `<div id="composer-option-${index}" class="composer-option${selected ? ' selected' : ''}" role="option" aria-selected="${selected}" data-composer-index="${index}"><strong>${escapeHtml(title)}</strong><small>${escapeHtml(detail || '')}</small>${openAction}</div>`
  }).join('')
  $('#composer-input').setAttribute('aria-activedescendant', `composer-option-${state.composerMenu.selected}`)
  menu.querySelector('.selected')?.scrollIntoView({ block: 'nearest' })
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
  loadSkillCatalog(cwd).then((skills) => {
    if (generation !== state.composerMenu.generation || state.composerMenu.type !== 'skill') return
    state.composerMenu.options = matchingSkills(trigger.query, skills)
    state.composerMenu.selected = 0
    renderComposerMenu()
  }).catch((error) => {
    if (generation !== state.composerMenu.generation) return
    renderComposerMenu(t('Failed to load skills: {message}', { message: error.message }))
  })
}

async function loadSkillCatalog(cwd, forceReload = false) {
  const catalog = state.skillCatalog
  if (!forceReload && catalog.cwd === cwd && catalog.loaded) return catalog.skills
  if (!forceReload && catalog.cwd === cwd && catalog.request) return catalog.request
  const request = rpc('skills/list', { cwds: cwd ? [cwd] : [], forceReload })
    .then((result) => (result?.data || []).flatMap((entry) => entry.skills || []).filter((skill) => skill.enabled))
  state.skillCatalog = { cwd, skills: [], request, loaded: false }
  try {
    const skills = await request
    if (state.skillCatalog.request === request) state.skillCatalog = { cwd, skills, request: null, loaded: true }
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
  if (state.composerMenu.type === 'file') {
    const replacement = replaceComposerTrigger(input.value, trigger, selectedFileReference(option))
    setCurrentComposerValue(replacement.value)
    input.setSelectionRange(replacement.cursor, replacement.cursor)
    if (state.backend === 'opencode' && state.selectedId) {
      const key = selectedStateKey()
      const files = state.pendingFiles[key] ||= []
      if (!files.some((file) => file.path === option.path)) files.push({ type: 'file', path: option.path, root: option.root })
    }
    hideComposerMenu()
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
  showCommandDialog('Model', '<div class="command-empty">Loading models from App Server…</div>')
  const models = await loadBackendModels({ refresh: true })
  const currentEffort = currentTurnOptions().effort
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
    const key = selectedStateKey()
    let options = currentTurnOptions()
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

async function loadBackendModels({ refresh = false } = {}) {
  const backend = state.backend
  if (!refresh && state.backendModels[backend].length) return state.backendModels[backend]
  if (!refresh && state.backendModelLoads.has(backend)) return state.backendModelLoads.get(backend)
  const load = rpc('model/list', { limit: 100, includeHidden: false })
    .then((result) => {
      const models = Array.isArray(result?.data) ? result.data : []
      state.backendModels[backend] = models
      if (state.backend === backend) renderComposerState()
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
  if (!state.selectedId || !skill?.name || !skill?.path) return
  const key = selectedStateKey()
  const skillsForThread = state.pendingSkills[key] ||= []
  if (!skillsForThread.some((candidate) => candidate.path === skill.path)) {
    skillsForThread.push({ type: 'skill', name: skill.name, path: skill.path })
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
  const active = Boolean(state.model.activeTurnId)
  const options = currentTurnOptions()
  const descriptor = currentBackend()
  const display = resolveModelDisplay({
    overrideModel: options.model,
    overrideEffort: options.effort,
    sessionModel: selectedThread()?.model,
    models: state.backendModels[state.backend],
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
  $('#queue-message').classList.toggle('hidden', !queueAvailable)
  $('#queue-message').textContent = queue.length >= state.queueDepth ? `${t('Queue')} (${queue.length}/${state.queueDepth})` : t('Queue')
  $('#continue-thread').classList.toggle('hidden', !continueAvailable)
  $('#continue-thread').textContent = draftingContinuation ? t('Drafting…') : t('Continue')
  $('#continue-thread').title = state.continueBehavior === 'quickSend'
    ? t('Immediately send a random continue prompt · Ctrl/Cmd+Shift+Enter')
    : state.continueBehavior === 'ollamaDraft'
      ? t('Draft the next message with local Ollama · Ctrl/Cmd+Shift+Enter')
      : t('Draft the next message with the current session model · Ctrl/Cmd+Shift+Enter')
  $('#continue-thread').disabled = !state.ready || !state.selectedId || hasComposerContent || Boolean(queue.length) || draftingContinuation
  $('#archive-thread').disabled = active || state.backend === 'opencode'
  $('#delete-thread').disabled = active
  $('#send-message').textContent = shellMode ? t('Run') : isRouterThread() ? t('Route') : active && isCodexBackend(state.backend) ? 'Steer' : 'Send'
  $('#send-message').classList.toggle('hidden', active && state.backend === 'opencode')
  $('#send-message').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode') || (shellMode && (active || !shellCommand))
  $('#composer-add-image').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode')
  renderMessageQueue()
  if (queue.length && !active && !state.pausedMessageQueues.has(selectedStateKey()) && !state.runningMessageQueues.has(selectedStateKey())) {
    const ref = { backend: state.backend, id: state.selectedId }
    queueMicrotask(() => runNextQueuedMessage(ref).catch((error) => console.error('Queued turn failed', error)))
  }
  reviewNotes.renderComposerContext()
}

function handleContinueAction() {
  if (state.continueBehavior === 'quickSend') {
    quickSendContinueMessage()
    return
  }
  draftContinueMessage()
}

function quickSendContinueMessage() {
  const button = $('#continue-thread')
  if (button.disabled || button.classList.contains('hidden')) return
  setCurrentComposerValue(randomContinuePrompt())
  hideComposerMenu()
  $('#composer-form').requestSubmit()
}

function composerHasPendingContent(key = selectedStateKey()) {
  const draft = key === selectedStateKey() ? $('#composer-input').value : composerDrafts.value(key)
  return Boolean(
    draft.trim()
      || state.pendingImages[key]?.length
      || state.pendingSkills[key]?.length
      || state.pendingFiles[key]?.length,
  )
}

async function draftContinueMessage() {
  const button = $('#continue-thread')
  if (button.disabled || button.classList.contains('hidden')) return
  const key = selectedStateKey()
  const source = latestAgentResponseText().trim()
  if (!source) {
    showError(new Error(t('This session has no {backend} response to continue.', { backend: currentBackend().name })))
    return
  }
  state.continuationDraftLoads.add(key)
  renderComposerState()
  try {
    const behavior = state.continueBehavior
    const prompt = behavior === 'ollamaDraft'
      ? await draftContinueWithOllama(source)
      : await draftContinueWithSessionModel(source, key)
    if (!prompt) throw new Error(t('The continuation backend returned an empty draft'))
    if (selectedStateKey() !== key) return
    if (state.model.activeTurnId || latestAgentResponseText().trim() !== source || composerHasPendingContent(key)) {
      toast(t('The conversation changed before the continuation draft was ready'))
      return
    }
    setComposerDraftValue(key, prompt)
    hideComposerMenu()
    const input = $('#composer-input')
    input.setSelectionRange(prompt.length, prompt.length)
    input.focus()
    toast(t('Continuation draft added to the composer'))
  } catch (error) {
    if (selectedStateKey() === key) showError(error)
  } finally {
    state.continuationDraftLoads.delete(key)
    renderComposerState()
  }
}

async function draftContinueWithOllama(source) {
  const response = await gatewayFetch('/studio/ollama/continue-draft', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      model: state.translation.ollamaModel || 'gemma3:4b',
      assistantResponse: truncateCharacters(source, 32_000),
    }),
  })
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) throw new Error(payload?.error?.message || t('Local Ollama could not draft a continuation'))
  return String(payload.prompt || '').trim()
}

async function draftContinueWithSessionModel(source, stateKey) {
  if (!state.ready || !state.selectedId) throw new Error(t('The current backend is not ready for continuation drafts'))
  const backend = state.backend
  const generation = state.socketGeneration
  const cwd = selectedThread()?.cwd || ''
  const options = currentTurnOptions()
  const selectedModel = selectedThread()?.model
  const model = String(options.model || (typeof selectedModel === 'string' ? selectedModel : '')).trim()
  const effort = String(options.effort || '').trim()
  const utilityName = `Studio continuation ${randomId()}`
  state.hiddenUtilityThreadNames.add(`${backend}:${utilityName}`)
  let threadId = ''
  let utilityTask = null
  try {
    ensureContinuationBackend(backend, generation, stateKey)
    const started = await rpc('thread/start', {
      cwd,
      ...(model ? { model } : {}),
      ...(isCodexBackend(backend) ? {
        ephemeral: true,
        approvalPolicy: 'never',
        sandbox: 'read-only',
        developerInstructions: CONTINUATION_DRAFT_INSTRUCTIONS,
      } : { name: utilityName }),
    }, 30_000)
    threadId = String(started?.thread?.id || '')
    if (!threadId) throw new Error(t('The current backend did not create a continuation draft task'))
    markUtilityThreadHidden(backend, threadId)
    ensureContinuationBackend(backend, generation, stateKey)
    if (isCodexBackend(backend)) {
      const utilityModel = createCodexViewModel()
      utilityModel.threadId = threadId
      utilityTask = { backend, threadId, turnId: '', model: utilityModel }
      state.structuredUtilityTasks.set(sessionRefKey(backend, threadId), utilityTask)
    }

    const turnStarted = await rpc('turn/start', {
      threadId,
      cwd,
      input: [{ type: 'text', text: continuationDraftInput(source) }],
      ...(!isCodexBackend(backend) ? { developerInstructions: CONTINUATION_DRAFT_INSTRUCTIONS } : {}),
      outputSchema: CONTINUATION_DRAFT_SCHEMA,
      ...(model ? { model } : {}),
      ...(effort ? { effort } : {}),
    }, 150_000)
    if (isCodexBackend(backend) && turnStarted?.turn?.id) {
      const turnId = String(turnStarted.turn.id)
      state.hiddenCodexTurns.add(routerRuntimeKey(backend, turnId))
      utilityTask.turnId ||= turnId
      if (!utilityTask.model.turns.some((turn) => String(turn.id) === turnId)) {
        applyCodexNotification(utilityTask.model, {
          method: 'turn/started',
          params: { threadId, turn: turnStarted.turn },
        })
      }
    }

    const draft = await waitForUtilityResult({
      ensureCurrent: () => ensureContinuationBackend(backend, generation, stateKey),
      read: async () => continuationDraftTurnState(isCodexBackend(backend)
        ? utilityTask?.model
        : (await rpc('thread/read', { threadId, includeTurns: true, cwd }, 30_000))?.thread),
      intervalMs: isCodexBackend(backend) ? 100 : 350,
      timeoutMs: 150_000, timeoutMessage: t('Continuation draft timed out'), errorMessage: t,
    })
    return draft.prompt
  } finally {
    if (threadId) state.structuredUtilityTasks.delete(sessionRefKey(backend, threadId))
    if (threadId) {
      dispatchBackendRpc(backend, 'thread/delete', {
        threadId,
        ...(!isCodexBackend(backend) ? { cwd } : {}),
      }, 15_000).catch((error) => {
        console.warn('Unable to remove the hidden continuation session', error)
      }).finally(() => {
        const threadKey = sessionRefKey(backend, threadId)
        state.hiddenUtilityThreads.delete(threadKey)
        state.hiddenCodexThreads.delete(threadKey)
        if (utilityTask?.turnId) state.hiddenCodexTurns.delete(routerRuntimeKey(backend, utilityTask.turnId))
        state.hiddenUtilityThreadNames.delete(`${backend}:${utilityName}`)
      })
    } else {
      state.hiddenUtilityThreadNames.delete(`${backend}:${utilityName}`)
    }
  }
}

function ensureContinuationBackend(backend, generation, stateKey) {
  if (state.backend !== backend || state.socketGeneration !== generation || !state.ready || selectedStateKey() !== stateKey) {
    throw new Error(t('Continuation draft stopped because the current session changed'))
  }
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

async function queueComposerMessage() {
  const key = selectedStateKey()
  const queue = state.messageQueues[key] || []
  const wasPaused = state.pausedMessageQueues.has(key)
  const previousError = state.messageQueueErrors.get(key)
  if (queue.length >= state.queueDepth) {
    $('#composer-message-queue').scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    return
  }
  const input = $('#composer-input')
  const text = input.value.trim()
  if (text.length > 64 * 1024) {
    showError(new Error(t('Queued messages must be 65,536 characters or fewer.')))
    return
  }
  const pendingImages = state.pendingImages[key] || []
  if (pendingImages.length) {
    showError(new Error(t('Image messages cannot be queued. Send or steer them directly.')))
    return
  }
  const skillInputs = [...(state.pendingSkills[key] || [])]
  const fileInputs = [...(state.pendingFiles[key] || [])]
  if (!text && !skillInputs.length && !fileInputs.length) return
  const message = {
    id: randomId(),
    text,
    input: [...skillInputs, ...fileInputs],
    createdAt: Date.now(),
  }
  state.messageQueues[key] = [...queue, message]
  if (!queue.length) {
    state.pausedMessageQueues.delete(key)
    state.messageQueueErrors.delete(key)
  }
  try {
    await persistMessageQueue(key)
  } catch (error) {
    state.messageQueues[key] = queue
    if (wasPaused) state.pausedMessageQueues.add(key)
    else state.pausedMessageQueues.delete(key)
    if (previousError) state.messageQueueErrors.set(key, previousError)
    else state.messageQueueErrors.delete(key)
    showError(error)
    renderComposerState()
    return
  }
  setComposerDraftValue(key, '')
  state.pendingSkills[key] = []
  state.pendingFiles[key] = []
  hideComposerMenu()
  renderComposerState()
  toast(t('Message queued'))
}

function handleMessageQueueClick(event) {
  if (state.runningMessageQueues.has(selectedStateKey())) return
  const edit = event.target.closest('[data-queue-edit]')
  if (edit) openQueuedMessageEditor(edit.dataset.queueEdit)
  const remove = event.target.closest('[data-queue-delete]')
  if (remove) deleteQueuedMessage(remove.dataset.queueDelete).catch(showError)
}

function openQueuedMessageEditor(id) {
  const key = selectedStateKey()
  const message = (state.messageQueues[key] || []).find((entry) => entry.id === id)
  if (!message) return
  editingQueuedMessage = { key, id }
  $('#edit-queued-message-text').value = message.text
  const attachmentCount = message.input.filter((entry) => entry.type !== 'text').length
  $('#edit-queued-message-note').textContent = attachmentCount ? t('{count} file or skill references will be kept.', { count: attachmentCount }) : ''
  $('#edit-queued-message-note').classList.toggle('hidden', !attachmentCount)
  $('#edit-queued-message-dialog').showModal()
  setTimeout(() => $('#edit-queued-message-text').focus(), 30)
}

function closeQueuedMessageEditor() {
  editingQueuedMessage = null
  $('#edit-queued-message-dialog').close()
}

async function saveEditedQueuedMessage(event) {
  event.preventDefault()
  if (!editingQueuedMessage) return
  const { key, id } = editingQueuedMessage
  const message = (state.messageQueues[key] || []).find((entry) => entry.id === id)
  if (!message) return closeQueuedMessageEditor()
  const text = $('#edit-queued-message-text').value.trim()
  if (!text && !message.input.length) return
  const previousText = message.text
  const previousInput = message.input
  message.text = text
  message.input = message.input.filter((entry) => entry.type !== 'text')
  try {
    await persistMessageQueue(key)
    closeQueuedMessageEditor()
    if (key === selectedStateKey()) renderComposerState()
  } catch (error) {
    message.text = previousText
    message.input = previousInput
    showError(error)
  }
}

async function deleteQueuedMessage(id) {
  const key = selectedStateKey()
  const previous = state.messageQueues[key] || []
  const wasPaused = state.pausedMessageQueues.has(key)
  const previousError = state.messageQueueErrors.get(key)
  state.messageQueues[key] = previous.filter((message) => message.id !== id)
  if (!state.messageQueues[key].length) {
    delete state.messageQueues[key]
    state.pausedMessageQueues.delete(key)
    state.messageQueueErrors.delete(key)
  }
  try { await persistMessageQueue(key) }
  catch (error) {
    state.messageQueues[key] = previous
    if (wasPaused) state.pausedMessageQueues.add(key)
    if (previousError) state.messageQueueErrors.set(key, previousError)
    throw error
  } finally { renderComposerState() }
}

function resumeSelectedMessageQueue() {
  const key = selectedStateKey()
  state.pausedMessageQueues.delete(key)
  state.messageQueueErrors.delete(key)
  renderComposerState()
  runNextQueuedMessage(sessionRefFromKey(key)).catch((error) => console.error('Queue resume failed', error))
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
  if (!ref) return
  const key = selectedStateKey(ref.id, ref.backend)
  if (!(state.messageQueues[key] || []).length) return
  state.pausedMessageQueues.add(key)
  if (reason) state.messageQueueErrors.set(key, reason)
  if (key === selectedStateKey()) renderComposerState()
}

function handleQueuedTurnCompletion(ref, status) {
  const key = selectedStateKey(ref.id, ref.backend)
  if (!(state.messageQueues[key] || []).length) return
  if (!completedQueueShouldAdvance(status, state.pausedMessageQueues.has(key))) {
    if (state.pausedMessageQueues.has(key)) return
    pauseMessageQueue(ref, t('Previous turn did not complete normally'))
    return
  }
  queueMicrotask(() => runNextQueuedMessage(ref).catch((error) => console.error('Queued turn failed', error)))
}

async function runNextQueuedMessage(ref) {
  if (!ref || !isSupportedBackend(ref.backend)) return
  const key = selectedStateKey(ref.id, ref.backend)
  const queue = state.messageQueues[key] || []
  if (!queue.length || state.pausedMessageQueues.has(key) || state.runningMessageQueues.has(key)) return
  const model = messageQueueModel(ref)
  if (model?.activeTurnId) return
  const message = queue[0]
  state.runningMessageQueues.add(key)
  state.messageQueueErrors.delete(key)
  let accepted = false
  let catalogActivity = null
  if (key === selectedStateKey()) {
    beginTranscriptFollowingLatest(model)
    renderComposerState()
  }
  try {
    const clientUserMessageId = randomId()
    catalogActivity = setCatalogThreadActivity(ref.backend, ref.id, { status: 'active', touch: true })
    const result = await startTurnWithPreparation({
      registry: sessionDispatch,
      ref,
      prepare: () => prepareComposerTurn(ref),
      start: () => dispatchBackendRpc(ref.backend, 'turn/start', turnStartParams(
        backendDescriptor(ref.backend).kind,
        threadForRef(ref),
        {
          threadId: ref.id,
          clientUserMessageId,
          input: [...(message.text ? [{ type: 'text', text: message.text }] : []), ...message.input],
          ...queuedTurnOptions(ref),
        },
      )),
      recoverThreadNotFound: isCodexBackend(ref.backend),
    })
    accepted = true
    if (result?.turn && model) applyTurnAcknowledgement(model, result.turn, ref.id)
    if (model) markCachedModelValidated(ref.backend, model)
    const current = state.messageQueues[key] || []
    if (current[0]?.id === message.id) current.shift()
    if (!current.length) {
      delete state.messageQueues[key]
      state.pausedMessageQueues.delete(key)
    }
    await persistMessageQueue(key)
    if (key === selectedStateKey()) {
      if (result?.turn) renderTranscript()
      toast(t('Queued message sent'))
    }
  } catch (error) {
    if (!accepted) rollbackCatalogThreadActivity(catalogActivity)
    if (accepted) {
      const current = state.messageQueues[key] || (state.messageQueues[key] = [])
      if (!current.some((entry) => entry.id === message.id)) current.unshift(message)
    }
    state.pausedMessageQueues.add(key)
    state.messageQueueErrors.set(key, accepted
      ? t('The message was accepted, but the queue could not be updated. Verify the conversation before resuming.')
      : String(error?.message || error))
    if (key === selectedStateKey()) showError(error)
    throw error
  } finally {
    state.runningMessageQueues.delete(key)
    if (key === selectedStateKey()) renderComposerState()
  }
}

async function sendComposer(event) {
  event.preventDefault()
  const input = $('#composer-input')
  const text = input.value.trim()
  const initialStateKey = selectedStateKey()
  const shellCommand = shellCommandFromComposer(input.value)
  if (shellCommand !== null) {
    if (!shellCommand || !state.selectedId) return
    if (state.model.activeTurnId) {
      showError(new Error('Wait for the current turn to finish or stop it before running a local shell command.'))
      return
    }
    const button = $('#send-message')
    button.disabled = true
    beginTranscriptFollowingLatest()
    try {
      await rpc('thread/shellCommand', { threadId: state.selectedId, command: shellCommand }, 120_000)
      setComposerDraftValue(initialStateKey, '')
      hideComposerMenu()
      renderComposerState()
      toast(t('Shell command sent to {backend}', { backend: currentBackend().name }))
    } catch (error) { showError(error) }
    finally { renderComposerState() }
    return
  }
  const slashName = text.match(/^\/([\w-]+)$/)?.[1]
  const slash = slashName && matchingSlashCommands(slashName).find((command) => command.name === slashName)
  if (slash) {
    setComposerDraftValue(initialStateKey, '')
    hideComposerMenu()
    try {
      await executeSlashCommand(slash.action)
    } catch (error) {
      showError(error)
    }
    return
  }
  if (!state.selectedId) return
  const stateKey = selectedStateKey()
  const pendingImages = [...(state.pendingImages[stateKey] || [])]
  const imageInputs = composerImageInputs(pendingImages)
  if (!text && !imageInputs.length) return
  if (isRouterThread()) {
    const button = $('#send-message')
    button.disabled = true
    beginTranscriptFollowingLatest()
    try {
      await threadRouter.startTurn(text, imageInputs)
      setComposerDraftValue(stateKey, '')
      state.pendingImages[stateKey] = []
      hideComposerMenu()
      renderComposerState()
      return true
    } catch (error) { showError(error) }
    finally { renderComposerState() }
    return false
  }
  const backend = state.backend
  const threadId = state.selectedId
  const targetModel = state.model
  const skillInputs = [...(state.pendingSkills[stateKey] || [])]
  const fileInputs = [...(state.pendingFiles[stateKey] || [])]
  const turnInput = [...(text ? [{ type: 'text', text }] : []), ...imageInputs, ...skillInputs, ...fileInputs]
  const turnOptions = configuredTurnOptions()
  const button = $('#send-message')
  button.disabled = true
  beginTranscriptFollowingLatest(targetModel)
  let optimisticTurnId = null
  let latencyTrace = null
  let composerCleared = false
  let catalogActivity = null
  let turnAccepted = false
  try {
    if (state.model.activeTurnId) {
      catalogActivity = setCatalogThreadActivity(backend, threadId, { status: 'active', touch: true })
      await rpc('turn/steer', {
        threadId,
        expectedTurnId: state.model.activeTurnId,
        clientUserMessageId: randomId(),
        input: turnInput,
      })
      turnAccepted = true
      toast('Message added to the current turn')
    } else {
      const clientUserMessageId = randomId()
      const ref = { backend, id: threadId }
      if (isCodexBackend(backend)) {
        optimisticTurnId = beginOptimisticCodexTurn(targetModel, { clientUserMessageId, input: turnInput })
        latencyTrace = beginTurnLatencyTrace(clientUserMessageId, threadId)
        setComposerDraftValue(stateKey, '')
        state.pendingSkills[stateKey] = []
        state.pendingFiles[stateKey] = []
        state.pendingImages[stateKey] = []
        composerCleared = true
        hideComposerMenu()
        renderComposerState()
        renderTranscript()
      }
      catalogActivity = setCatalogThreadActivity(backend, threadId, { status: 'active', touch: true })
      const result = await startTurnWithPreparation({
        registry: sessionDispatch,
        ref,
        prepare: () => prepareComposerTurn(ref),
        start: () => dispatchBackendRpc(backend, 'turn/start', turnStartParams(
          backendDescriptor(backend).kind,
          threadForRef(ref),
          {
            threadId,
            clientUserMessageId,
            input: turnInput,
            ...turnOptions,
          },
        )),
        recoverThreadNotFound: isCodexBackend(backend),
      })
      turnAccepted = true
      if (result?.turn) {
        if (isCodexBackend(backend) && optimisticTurnId) {
          reconcileOptimisticCodexTurn(targetModel, optimisticTurnId, result.turn)
          bindTurnLatencyTrace(latencyTrace, result.turn.id)
          markTurnLatency(latencyTrace, 'turn_start_ack')
        } else applyTurnAcknowledgement(targetModel, result.turn, threadId)
        markCachedModelValidated(backend, targetModel)
        if (targetModel === state.model) renderTranscript()
      }
    }
    if (!composerCleared) {
      setComposerDraftValue(stateKey, '')
      state.pendingSkills[stateKey] = []
      state.pendingFiles[stateKey] = []
      state.pendingImages[stateKey] = []
    }
    hideComposerMenu()
    renderComposerState()
  } catch (error) {
    if (!turnAccepted) rollbackCatalogThreadActivity(catalogActivity)
    if (optimisticTurnId) {
      rollbackOptimisticCodexTurn(targetModel, optimisticTurnId)
      finishTurnLatencyTrace(latencyTrace, 'failed')
      if (targetModel === state.model) renderTranscript()
    }
    if (composerCleared) {
      if (!composerDrafts.value(stateKey).trim()) setComposerDraftValue(stateKey, text)
      if (!(state.pendingSkills[stateKey] || []).length) state.pendingSkills[stateKey] = skillInputs
      if (!(state.pendingFiles[stateKey] || []).length) state.pendingFiles[stateKey] = fileInputs
      if (!(state.pendingImages[stateKey] || []).length) state.pendingImages[stateKey] = pendingImages
      if (state.backend === backend && state.selectedId === threadId) renderComposerState()
    }
    showError(error)
  }
  finally { renderComposerState() }
  return turnAccepted
}

function isRouterThread(threadId = state.selectedId, backend = state.backend) {
  return threadRouter.isThread(threadId, backend)
}

async function ensureSessionModel(ref) {
  const key = threadCatalogKey(ref.backend, ref.id)
  const historyEpoch = ref.backend === 'opencode' ? openCodeHistoryEpoch : null
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
      if (historyEvents.length) replayOpenCodeEventsAfterHistory(model, historyEvents, ref.id, {
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

function mergeThreadIntoCatalog(backend, incoming) {
  if (!incoming?.id) return
  const catalog = state.threadsByBackend[backend] || (state.threadsByBackend[backend] = [])
  const metadata = { ...incoming, turns: undefined }
  const index = catalog.findIndex((thread) => thread.id === incoming.id)
  if (index >= 0) catalog[index] = { ...catalog[index], ...metadata }
  else catalog.unshift(metadata)
  if (backend === state.backend) state.threads = catalog
}

function activateStartedThread(backend, thread, { operation = 'new' } = {}) {
  if (!thread?.id) throw new Error(t('The backend created a session without an ID.'))
  if (backend !== state.backend || !state.ready) {
    throw new Error(t('The new session backend changed before Studio could open it.'))
  }

  rememberStartedThread(backend, thread, operation)
  const model = createCodexViewModel()
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
  $('#native-connection').textContent = t('Connected')
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
      catalogContains: Boolean(threadForRef({ backend, id: thread.id })),
    })
    return thread.id
  }).catch((error) => {
    reportSessionLifecycle('activation-failed', {
      backend,
      threadId: thread.id,
      operation,
      error: error?.message || String(error),
    })
    throw error
  })
}

async function interruptTurn() {
  if (!state.selectedId || !state.model.activeTurnId) return
  pauseMessageQueue({ backend: state.backend, id: state.selectedId })
  try {
    await rpc('turn/interrupt', { threadId: state.selectedId, turnId: state.model.activeTurnId })
    toast('Requested interruption of the current turn')
  } catch (error) { showError(error) }
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
    errorBox.textContent = error.message
    errorBox.classList.remove('hidden')
  }
}

async function createThread(event) {
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
  const params = {
    cwd,
    ...(isCodexBackend(backend) ? {
      approvalPolicy: $('#new-thread-approval').value,
      sandbox: $('#new-thread-sandbox').value,
    } : {}),
  }
  const model = $('#new-thread-model').value.trim()
  const supportedModels = state.backendModels[backend].map((entry) => entry.model || entry.id).filter(Boolean)
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
      error: error?.message || String(error),
    })
    if (createdThreadId) showError(error)
    else {
      errorBox.textContent = error.message
      errorBox.classList.remove('hidden')
    }
  } finally { button.disabled = false }
}

async function forkThread(lastTurnId = null, trigger = null) {
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

async function forkSelectedThread() {
  await forkThread()
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

async function loadPreferences() {
  const loadJson = async (path, label) => {
    try {
      const response = await gatewayFetch(path, { cache: 'no-store' })
      if (response.ok) return response.json()
      console.warn(`Unable to load ${label}: HTTP ${response.status}`)
    } catch (error) { console.warn(`Unable to load ${label}`, error) }
    return {}
  }
  const [saved, storedSessionState] = await Promise.all([
    loadJson('/studio/preferences', 'preferences'),
    loadJson('/studio/session-state', 'session state'),
  ])
  state.language = normalizeLanguage(saved.language)
  state.theme = saved.theme === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth(saved.contentWidth)
  state.hiddenSessionDirectories = normalizeHiddenSessionDirectories(saved.hiddenSessionDirectories)
  state.sharedDocumentDirectories = normalizeSharedDocumentDirectories(saved.sharedDocumentDirectories)
  state.sessionDirectoryIgnore = Array.isArray(saved.sessionDirectoryIgnore)
    ? saved.sessionDirectoryIgnore.map((value) => String(value)).filter((value) => value.length <= 4096)
    : []
  state.wsl = {
    distribution: String(saved.wslDistribution || '').trim(),
    user: String(saved.wslUser || '').trim(),
    codexBinary: String(saved.wslCodexBinary || 'codex').trim() || 'codex',
    opencodeBinary: String(saved.wslOpencodeBinary || 'opencode').trim() || 'opencode',
  }
  state.sidebarCollapsed = Boolean(saved.sidebarCollapsed)
  state.rightRailWidthRatio = normalizeRightRailWidthRatio(saved.rightRailWidthRatio ?? saved.artifactWidthRatio)
  state.typography = normalizeTypography({ ...typographyDefaults, ...migrateDefaultFontFamilies(saved.typography) })
  state.mermaid = normalizeMermaidPreferences(saved.mermaid)
  state.markdown = { mode: ['reading', 'technical', 'compact'].includes(saved.markdown?.mode) ? saved.markdown.mode : 'technical' }
  state.translation = normalizeTranslationPreferences(saved.translation)
  state.desktopNotifications = Boolean(saved.desktopNotifications)
  state.queueDepth = normalizeQueueDepth(saved.queueDepth)
  state.continueBehavior = normalizeContinueBehavior(saved.continueBehavior)
  state.browser = {
    enabled: true,
    restoreTabs: false,
    allowHttp: true,
    allowPrivateNetwork: false,
    allowLocalhost: true,
    previewJavaScript: true,
    embeddedWidth: 720,
    agent: { enabled: false, provider: 'playwright-mcp', profile: 'persistent', approval: 'interactive', allowedOrigins: [] },
    ...(saved.browser || {}),
  }
  state.router = normalizeThreadRouter(saved.router)
  state.backend = isSupportedBackend(state.router.controllerBackend)
    ? state.router.controllerBackend
    : 'codex'
  state.selectedByBackend = emptyBackendSelections()
  const routerId = state.router.controllers[state.backend]
  if (typeof routerId === 'string' && routerId) state.selectedByBackend[state.backend] = routerId
  state.startupRouterSelectionPending = true
  state.attentionThreads = new Set()
  state.selectedId = state.selectedByBackend[state.backend]
  state.turnOptions = normalizeStoredTurnOptions(storedSessionState.turnOptions)
  state.messageQueues = normalizeStoredMessageQueues(storedSessionState.messageQueues)
  state.pausedMessageQueues = new Set(Object.keys(state.messageQueues))
  state.runningMessageQueues = new Set()
  state.messageQueueErrors = new Map()
  state.annotationDrafts = normalizeAnnotationDrafts(storedSessionState.annotationDrafts)
  state.annotationAdditional = normalizeAdditional(storedSessionState.annotationAdditional)
  state.pinnedSessions = new Set(
    Array.isArray(storedSessionState.pinnedSessions)
      ? storedSessionState.pinnedSessions.filter((key) => typeof key === 'string' && key.includes(':')).slice(0, 10)
      : [],
  )
  const initialLocale = resolveLanguage(state.language)
  state.annotationPromptTemplates = normalizeLocalizedTemplates(saved.annotationPromptTemplates)
  if (!state.annotationPromptTemplates[initialLocale]) {
    state.annotationPromptTemplates[initialLocale] = defaultAnnotationPrompt(initialLocale)
  }
  state.activeAnnotationPromptTemplate = state.annotationPromptTemplates[initialLocale]
  const storedOpeningMessages = normalizeOpeningMessages(storedSessionState.openingMessages)
  state.openingMessages = migrateLegacyResponsibilities(storedOpeningMessages, saved.router)
  preferencesReady = true
  for (const [key, message] of Object.entries(state.openingMessages)) {
    if (JSON.stringify(message) !== JSON.stringify(storedOpeningMessages[key])) {
      persistOpeningMessageState(key)
    }
  }
  applySidebarState()
}

function preferencesSnapshot() {
  return {
    language: state.language,
    theme: state.theme,
    contentWidth: state.contentWidth,
    hiddenSessionDirectories: state.hiddenSessionDirectories,
    sharedDocumentDirectories: state.sharedDocumentDirectories,
    sessionDirectoryIgnore: state.sessionDirectoryIgnore,
    wslDistribution: state.wsl.distribution || null,
    wslUser: state.wsl.user || null,
    wslCodexBinary: state.wsl.codexBinary || 'codex',
    wslOpencodeBinary: state.wsl.opencodeBinary || 'opencode',
    sidebarCollapsed: state.sidebarCollapsed,
    rightRailWidthRatio: state.rightRailWidthRatio,
    typography: state.typography,
    mermaid: state.mermaid,
    markdown: state.markdown,
    translation: state.translation,
    desktopNotifications: state.desktopNotifications,
    queueDepth: state.queueDepth,
    continueBehavior: state.continueBehavior,
    browser: state.browser,
    annotationPromptTemplates: state.annotationPromptTemplates,
    router: Object.keys(state.router.controllers).length || state.router.fallbacks.length ? state.router : null,
  }
}

function persistPreferences() {
  if (!preferencesReady) return Promise.resolve()
  return preferencesWriter.write('/studio/preferences', preferencesSnapshot())
}

function queueSessionStateWrite(path, body, method = 'PUT') {
  if (!preferencesReady) return Promise.resolve()
  return sessionStateWriter.write(path, body, method)
}

function persistAnnotationState(key) {
  if (!key) return Promise.resolve()
  return queueSessionStateWrite('/studio/session-state/annotations', {
    sessionKey: key,
    drafts: state.annotationDrafts[key] || [],
    additional: state.annotationAdditional[key] || '',
  })
}

function persistOpeningMessageState(key) {
  if (!key) return Promise.resolve()
  return queueSessionStateWrite('/studio/session-state/opening-message', {
    sessionKey: key,
    message: state.openingMessages[key] || null,
  })
}

function persistSessionTurnOptions(key) {
  if (!key) return Promise.resolve()
  const options = state.turnOptions[key] || {}
  return queueSessionStateWrite('/studio/session-state/turn-options', sessionModelPreferencePayload(key, options))
}

function persistMessageQueue(key) {
  if (!key) return Promise.resolve()
  return queueSessionStateWrite('/studio/session-state/message-queue', {
    sessionKey: key,
    messages: state.messageQueues[key] || [],
  })
}

function deletePersistedSessionState(key) {
  if (!key) return Promise.resolve()
  return queueSessionStateWrite('/studio/session-state/session', { sessionKey: key }, 'DELETE')
}

function persistSessionPin(key, pinned) {
  if (!key) return Promise.resolve()
  return queueSessionStateWrite('/studio/session-state/pin', { sessionKey: key, pinned })
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

function normalizeSharedDocumentDirectories(values = []) {
  if (!Array.isArray(values)) return []
  return [...new Set(values
    .map((value) => String(value || '').trim())
    .filter((value) => value && value.length <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)))]
    .slice(0, 256)
}

function isAbsoluteDocumentDirectory(value) {
  return value.startsWith('/') || /^[a-z]:[\\/]/iu.test(value) || /^\\\\[^\\]/u.test(value)
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

async function saveSettings(event) {
  event.preventDefault()
  const template = $('#annotation-template').value.trim()
  if (!template.includes('{{annotations}}')) {
    $('#settings-error').textContent = 'The comment template must contain {{annotations}}.'
    $('#settings-error').classList.remove('hidden')
    return
  }
  const previousLocale = getLocale()
  const sharedDocumentDirectories = normalizeSharedDocumentDirectories(
    $('#shared-document-directories').value.split(/\r?\n/u),
  )
  if (sharedDocumentDirectories.some((directory) => !isAbsoluteDocumentDirectory(directory))) {
    $('#settings-error').textContent = t('Shared document directories must use absolute paths.')
    $('#settings-error').classList.remove('hidden')
    return
  }
  state.annotationPromptTemplates[previousLocale] = template.slice(0, 32000)
  state.language = normalizeLanguage($('#language-select').value)
  setLanguage(state.language)
  syncEmbeddedBrowserTranslations()
  state.theme = $('#theme-select').value === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth($('#content-width').value)
  state.queueDepth = normalizeQueueDepth($('#queue-depth').value)
  state.continueBehavior = normalizeContinueBehavior($('#continue-behavior').value)
  state.sharedDocumentDirectories = sharedDocumentDirectories
  const translationModel = $('#translation-model').value.trim().slice(0, 256)
  const translationEffort = $('#translation-effort').value
  state.translation.engine = $('#translation-engine').value === 'ollama' ? 'ollama' : 'backend'
  state.translation.ollamaModel = $('#translation-ollama-model').value.trim().slice(0, 256) || 'gemma3:4b'
  if (translationModel) state.translation.models[state.backend] = translationModel
  else delete state.translation.models[state.backend]
  if (translationEffort) state.translation.efforts[state.backend] = translationEffort
  else delete state.translation.efforts[state.backend]
  state.typography = normalizeTypography({
    uiFontFamily: $('#ui-font-family').value.trim(),
    uiFontSize: Number($('#ui-font-size').value),
    uiFontWeight: Number($('#ui-font-weight').value),
    contentFontFamily: $('#content-font-family').value.trim(),
    contentFontSize: Number($('#content-font-size').value),
    contentFontWeight: Number($('#content-font-weight').value),
    codeFontFamily: $('#code-font-family').value.trim(),
    codeFontSize: Number($('#code-font-size').value),
    codeFontWeight: Number($('#code-font-weight').value),
    highContrast: $('#high-contrast').checked,
  })
  const wantsNotifications = $('#desktop-notifications').checked
  if (wantsNotifications && 'Notification' in window && Notification.permission === 'default') {
    await Notification.requestPermission().catch(() => 'denied')
  }
  state.desktopNotifications = wantsNotifications && (!('Notification' in window) || Notification.permission !== 'denied')
  const previousWsl = JSON.stringify(state.wsl)
  if (state.hostPlatform === 'windows') {
    state.wsl = {
      distribution: $('#wsl-distribution').value.trim(),
      user: $('#wsl-user').value.trim(),
      codexBinary: $('#wsl-codex-binary').value.trim() || 'codex',
      opencodeBinary: $('#wsl-opencode-binary').value.trim() || 'opencode',
    }
  }
  const nextLocale = getLocale()
  state.activeAnnotationPromptTemplate = nextLocale === previousLocale
    ? template.slice(0, 32000)
    : state.annotationPromptTemplates[nextLocale] || defaultAnnotationPrompt(nextLocale)
  state.annotationPromptTemplates[nextLocale] = state.activeAnnotationPromptTemplate
  applyAppearance()
  try {
    await persistPreferences()
  } catch (error) {
    $('#settings-error').textContent = t('Unable to save settings')
    $('#settings-error').classList.remove('hidden')
    return
  }
  sessionResources.invalidate(state.backend, state.selectedId, { force: true })
  $('#settings-dialog').close()
  renderLocalizedUI()
  if (state.hostPlatform === 'windows' && JSON.stringify(state.wsl) !== previousWsl) {
    toast(t('WSL settings saved. Restart Studio to apply them.'))
  }
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
  const secrets = parseEnvironmentLines($('#environment-secrets').value, { allowEmpty: false })
  const removeSecrets = [...environmentSecretRemovals]
  for (const name of removeSecrets) {
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,127}$/u.test(name)) throw new Error(`Invalid secret name: ${name}`)
  }
  const variables = parseEnvironmentLines($('#environment-variables').value, { allowEmpty: true })
  const allowedHosts = parseHosts($('#environment-allowed-hosts').value)
  const cacheVariables = parseEnvironmentLines($('#environment-cache-variables').value, { allowEmpty: false })
  const networkPolicy = selectedEnvironmentPolicy()
  if (!environmentDialogProfile.configured && !Object.keys(variables).length && !Object.keys(secrets).length && !removeSecrets.length && !allowedHosts.length && !Object.keys(cacheVariables).length && networkPolicy === 'restricted') return
  const response = await gatewayFetch('/studio/environment', {
    method: 'PUT', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      root,
      variables,
      secrets, removeSecrets,
      networkPolicy,
      allowedHosts,
      cacheVariables,
    }),
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
  if (!isCodexBackend(backend) || !threadId) return null
  const generation = state.appServerGenerations[backend] ?? 'unknown'
  const key = `${backend}\u0000${threadId}\u0000${generation}\u0000${root}\u0000${profileRevision}`
  if (!force && appliedEnvironmentProfiles.has(key)) return null
  if (!force && environmentApplyRequests.has(key)) return environmentApplyRequests.get(key)
  const request = (async () => {
    const apply = async (incremental) => {
      const response = await gatewayFetch('/studio/environment/apply', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          root,
          threadId,
          backend,
          includeThread,
          excludeTurns: incremental,
          ...(incremental && initialTurnsPage ? { initialTurnsPage } : {}),
        }),
      })
      const result = await response.json().catch(() => null)
      if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
      return result
    }
    let result
    try {
      result = await apply(excludeTurns)
    } catch (error) {
      if (!excludeTurns || !isHistoryPaginationCompatibilityError(error)) throw error
      rememberHistoryTailCapability(backend, threadId, false)
      result = await apply(false)
    }
    appliedEnvironmentProfiles.add(key)
    return result
  })().finally(() => environmentApplyRequests.delete(key))
  environmentApplyRequests.set(key, request)
  return request
}

function resetSettings() {
  state.language = 'system'
  setLanguage(state.language)
  syncEmbeddedBrowserTranslations()
  state.theme = 'light'
  state.contentWidth = 'comfortable'
  state.typography = { ...typographyDefaults }
  state.translation = normalizeTranslationPreferences(null)
  state.desktopNotifications = false
  state.queueDepth = 1
  state.continueBehavior = 'sessionModelDraft'
  state.wsl = { distribution: '', user: '', codexBinary: 'codex', opencodeBinary: 'opencode' }
  state.annotationPromptTemplates = { ...annotationPromptDefaults }
  state.activeAnnotationPromptTemplate = defaultAnnotationPrompt()
  populateSettingsForm()
  applyAppearance()
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

function normalizeTranslationPreferences(value) {
  const source = value && typeof value === 'object' ? value : {}
  const engine = source.engine === 'ollama' ? 'ollama' : 'backend'
  const candidateOllamaModel = String(source.ollamaModel || '').trim()
  const ollamaModel = candidateOllamaModel
    && candidateOllamaModel.length <= 256
    && !/[\u0000-\u001f]/u.test(candidateOllamaModel)
    ? candidateOllamaModel
    : 'gemma3:4b'
  const models = {}
  for (const [backend, model] of Object.entries(source.models || {})) {
    const normalized = String(model || '').trim()
    if (isSupportedBackend(backend) && normalized && normalized.length <= 256 && !/[\u0000-\u001f]/u.test(normalized)) {
      models[backend] = normalized
    }
  }
  const efforts = {}
  const supportedEfforts = new Set(['minimal', 'low', 'medium', 'high', 'xhigh'])
  for (const [backend, effort] of Object.entries(source.efforts || {})) {
    if (isSupportedBackend(backend) && supportedEfforts.has(effort)) efforts[backend] = effort
  }
  for (const backend of BACKEND_IDS) {
    if (isCodexBackend(backend) && !efforts[backend]) efforts[backend] = 'low'
  }
  return { engine, ollamaModel, models, efforts }
}

function normalizeContinueBehavior(value) {
  return ['ollamaDraft', 'quickSend'].includes(value) ? value : 'sessionModelDraft'
}


function normalizeTypography(value) {
  const weights = [400, 500, 600]
  const uiFontFamily = String(value.uiFontFamily || typographyDefaults.uiFontFamily).trim().slice(0, 512) || typographyDefaults.uiFontFamily
  const uiFontWeight = weights.includes(Number(value.uiFontWeight)) ? Number(value.uiFontWeight) : typographyDefaults.uiFontWeight
  return {
    uiFontFamily,
    uiFontSize: Math.min(20, Math.max(11, Number(value.uiFontSize) || typographyDefaults.uiFontSize)),
    uiFontWeight,
    contentFontFamily: String(value.contentFontFamily || uiFontFamily).trim().slice(0, 512) || uiFontFamily,
    contentFontSize: Math.min(24, Math.max(11, Number(value.contentFontSize) || typographyDefaults.contentFontSize)),
    contentFontWeight: weights.includes(Number(value.contentFontWeight)) ? Number(value.contentFontWeight) : uiFontWeight,
    codeFontFamily: String(value.codeFontFamily || typographyDefaults.codeFontFamily).trim().slice(0, 512) || typographyDefaults.codeFontFamily,
    codeFontSize: Math.min(20, Math.max(11, Number(value.codeFontSize) || typographyDefaults.codeFontSize)),
    codeFontWeight: weights.includes(Number(value.codeFontWeight)) ? Number(value.codeFontWeight) : typographyDefaults.codeFontWeight,
    highContrast: value.highContrast === undefined ? typographyDefaults.highContrast : Boolean(value.highContrast),
  }
}

function migrateDefaultFontFamilies(value) {
  const typography = value && typeof value === 'object' ? { ...value } : {}
  if (legacyDefaultUiFontFamilies.has(typography.uiFontFamily)) typography.uiFontFamily = defaultUiFontFamily
  if (!typography.contentFontFamily && typography.uiFontFamily) typography.contentFontFamily = typography.uiFontFamily
  if (!typography.contentFontWeight && typography.uiFontWeight) typography.contentFontWeight = typography.uiFontWeight
  if (legacyDefaultUiFontFamilies.has(typography.contentFontFamily)) typography.contentFontFamily = defaultUiFontFamily
  return typography
}

function normalizeContentWidth(value) {
  return ['comfortable', 'wide', 'full'].includes(value) ? value : 'comfortable'
}

function normalizeLanguage(value) {
  return ['system', 'zh-CN', 'en-US'].includes(value) ? value : 'system'
}

function normalizeAnnotationDrafts(value) {
  return normalizeCommentDrafts(value, {
    idFactory: randomId,
    migrateSource: legacyCommentSource,
    registry: commentSources,
  })
}

function normalizeAdditional(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).flatMap(([id, text]) => id && typeof text === 'string' ? [[id.includes(':') ? id : `codex:${id}`, text.slice(0, 32000)]] : []))
}
function normalizeOpeningMessages(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).slice(0, 2048).flatMap(([key, message]) => {
    if (!key || !message || typeof message !== 'object') return []
    const text = truncateUtf8(String(message.text || '').trim(), 16 * 1024)
    const responsibility = truncateCharacters(String(message.responsibility || '').trim(), 4096)
    return text || responsibility ? [[key.includes(':') ? key : `codex:${key}`, {
      text,
      responsibility,
      source: String(message.source || 'history').slice(0, 64),
      capturedAt: String(message.capturedAt || new Date().toISOString()).slice(0, 128),
      truncated: Boolean(message.truncated),
    }]] : []
  }))
}
function toast(message, kind = 'info') {
  const element = document.createElement('div')
  element.className = `toast ${kind}`
  element.textContent = t(message)
  $('#toast-region').appendChild(element)
  setTimeout(() => element.remove(), 3200)
}
function showError(error) { console.error(error); reportClientError(error); toast(error?.message || String(error), 'error') }
