import {
  applyCodexNotification,
  beginOptimisticCodexTurn,
  createCodexViewModel,
  hydrateCodexThread,
  reconcileOptimisticCodexTurn,
  resolveCodexApproval,
  resolveCodexInteraction,
  rollbackOptimisticCodexTurn,
  selectedThreadStatusChange,
  textFromUserContent,
} from './codex-native.mjs'
import {
  applyOpenCodeEvent,
  collectOpenCodeRootSessions,
  fetchOpenCodeDirectoryStatuses,
  normalizeOpenCodeSessions,
  openCodeMessageId,
  openCodeModelList,
  openCodeThreadFromHistory,
  splitOpenCodeModel,
} from './opencode-native.mjs'
import { resolveModelDisplay } from './model-display.mjs'
import {
  catalogListParams,
  mergeCatalogMetadata,
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
  turnNavigationLabel,
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
  annotationPromptDefaults,
  getLocale,
  migrateLocalizedTemplates,
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
} from './transcript-scroll.mjs'
import { createWorkspaceTools } from './workspace-tools.mjs'
import { createSessionResourcesUI } from './session-resources-ui.mjs'
import { rightRailWidthBounds } from './right-rail-layout.mjs'
import {
  catalogCountsWithAttention,
  catalogTimestamp,
  filterCatalogEntries,
  groupCatalogEntries,
  isSessionDirectoryHidden,
  normalizeHiddenSessionDirectories,
  threadCatalogKey,
} from './thread-catalog.mjs'
import {
  addLoadedThread,
  updateLoadedCatalogTimestamp,
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
import { SessionDispatchRegistry } from './session-dispatch.mjs'
import DOMPurify from './vendor/purify.es.mjs'
import { formatEnvironmentLines, parseEnvironmentLines, parseHosts } from './environment-profile.mjs'
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
    prepareTurn: (ref) => dispatchBackendRpc(ref.backend, 'thread/resume', { threadId: ref.id }),
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
    startTurn: async (ref, input, options = {}) => {
      const clientUserMessageId = openCodeMessageId(options.clientUserMessageId || randomId())
      await dispatchBackendRpc(ref.backend, 'turn/start', {
        threadId: ref.id,
        clientUserMessageId,
        input,
        ...(options.developerInstructions ? { developerInstructions: options.developerInstructions } : {}),
        ...(options.outputSchema ? { outputSchema: options.outputSchema } : {}),
        ...(options.turnOptions || {}),
      }, options.timeoutMs)
      return {
        turn: {
          id: clientUserMessageId,
          status: 'inProgress',
          items: [{ id: clientUserMessageId, type: 'userMessage', content: input }],
        },
      }
    },
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
  socket: null,
  eventSource: null,
  socketGeneration: 0,
  reconnectTimer: null,
  ready: false,
  backendInfo: null,
  backendInfos: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null])),
  backendModels: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, []])),
  backendRegistry: { configPath: '', configurationError: null },
  backendModelLoads: new Map(),
  hostPlatform: window.__CODEX_THREAD_STUDIO_GATEWAY__?.hostPlatform || null,
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
  selectedId: null,
  search: '',
  filter: 'all',
  sessionLibrary: createSessionLibraryState(),
  threadSearch: createThreadSearchState(),
  attentionThreads: new Set(),
  collapsedThreadGroups: new Set(),
  model: createCodexViewModel(),
  language: 'system',
  theme: 'light',
  contentWidth: 'comfortable',
  hiddenSessionDirectories: [],
  sessionDirectoryIgnore: [],
  sidebarCollapsed: false,
  rightRailWidthRatio: 0.44,
  typography: { ...typographyDefaults },
  mermaid: { ...MERMAID_PREFERENCES_DEFAULTS },
  markdown: { mode: 'technical' },
  desktopNotifications: false,
  appServerCapabilities: {},
  appServerInitialization: null,
  appServerGenerations: Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null])),
  environmentProfile: null,
  browser: null,
  browserInfo: null,
  embeddedBrowserVisible: false,
  embeddedBrowserLoaded: false,
  activeRightWorkspace: null,
  ...createReviewNotesState(),
  annotationPromptTemplates: {},
  annotationPromptTemplate: annotationPromptDefaults['en-US'],
  openingMessages: {},
  ...createDocumentWorkspaceState(),
  composerMenu: { type: null, trigger: null, options: [], selected: 0, generation: 0 },
  skillCatalog: { cwd: null, skills: [], request: null, loaded: false },
  turnOptions: {},
  pendingSkills: {},
  pendingFiles: {},
  pendingImages: {},
  ...createSessionMapRuntimeState(),
  hiddenCodexThreads: new Set(),
  hiddenCodexTurns: new Set(),
  hiddenUtilityThreads: new Set(),
  hiddenUtilityThreadNames: new Set(),
  selectionTranslationCache: new Map(),
  router: normalizeThreadRouter(null),
  routerRuntime: createThreadRouterRuntimeState(),
}

let preferencesReady = false
let preferencesWriteChain = Promise.resolve()
let transcriptFrame = null
const dirtyStreamItems = new Map()
const turnLatencyTraces = new Map()
let composerSearchTimer = null
let codexCatalogFocusRefreshAt = 0
let artifactMarkdownImageObserver = null
let turnNavigatorFrame = null
let openCodeListRefreshTimer = null
const openCodeStatusReconcileTimers = new Map()
let threadCatalogRetryTimer = null
let threadCatalogRetryAttempt = 0
let threadCatalogErrorMessage = null
const catalogRefreshes = new Map()
const catalogRequestGenerations = new Map()
const codexCatalogRecoveryStarted = new Set()
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
let pendingTranscriptViewRestore = null

const workspaceTools = createWorkspaceTools({
  gatewayFetch,
  gatewayWebSocket,
  getThread: selectedThread,
  getBackend: () => state.backend,
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
    if (state.activeRightWorkspace === tool) state.activeRightWorkspace = null
    syncRightWorkspaceLaunchers()
    sessionMap.render()
  },
  openResource: openSessionResource,
  openSource: openSessionResourceSource,
  favoriteResource: (resource, occurrence) => reviewNotes.openFavoriteForResource(resource, occurrence),
  translate: t,
  notify: toast,
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
    pauseFollowing: () => transcriptScrollFollower.pause(),
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
  persistPreferences,
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
    pauseTranscript: () => transcriptScrollFollower.pause(),
    switchBackend,
    waitForBackend: waitFor,
    loadThreads,
    selectThread,
    translateSelection: translateSelectionWithCurrentBackend,
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

document.addEventListener('DOMContentLoaded', () => init().catch(showError))
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
  $('#turn-navigator-list').addEventListener('click', handleTurnNavigatorClick)
  $('#open-browser-workspace').addEventListener('click', () => openGlobalBrowser().catch(showError))
  $('#transcript').addEventListener('scroll', handleTranscriptScroll, { passive: true })
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
  if (!usesEmbeddedBrowser()) throw new Error(t('The embedded browser is unavailable on this platform'))
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
  if (!state.ready || !state.selectedId) throw new Error(t('The current backend is not ready for translation'))

  const backend = state.backend
  const generation = state.socketGeneration
  const cwd = selectedThread()?.cwd || ''
  const options = { ...currentTurnOptions() }
  const model = options.model || selectedThread()?.model || ''
  const cacheKey = translationCacheKey({ backend, model, effort: options.effort, text })
  const cached = state.selectionTranslationCache.get(cacheKey)
  if (cached) {
    state.selectionTranslationCache.delete(cacheKey)
    state.selectionTranslationCache.set(cacheKey, cached)
    return cached
  }

  const utilityName = `Studio translation ${randomId()}`
  state.hiddenUtilityThreadNames.add(`${backend}:${utilityName}`)
  let threadId = ''
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

    const turnStarted = await rpc('turn/start', {
      threadId,
      cwd,
      input: [{ type: 'text', text: selectionTranslationInput(text) }],
      ...(!isCodexBackend(backend) ? { developerInstructions: SELECTION_TRANSLATION_INSTRUCTIONS } : {}),
      outputSchema: SELECTION_TRANSLATION_SCHEMA,
      ...(model ? { model } : {}),
      ...(options.effort ? { effort: options.effort } : {}),
    }, 150_000)
    if (isCodexBackend(backend) && turnStarted?.turn?.id) {
      state.hiddenCodexTurns.add(routerRuntimeKey(backend, turnStarted.turn.id))
    }

    const deadline = Date.now() + 150_000
    while (Date.now() < deadline) {
      ensureTranslationBackend(backend, generation)
      const result = await rpc('thread/read', {
        threadId,
        includeTurns: true,
        ...(!isCodexBackend(backend) ? { cwd } : {}),
      }, 30_000)
      const translation = translationTurnState(result?.thread)
      if (translation.status === 'completed') {
        state.selectionTranslationCache.set(cacheKey, translation.translation)
        while (state.selectionTranslationCache.size > 64) {
          state.selectionTranslationCache.delete(state.selectionTranslationCache.keys().next().value)
        }
        return translation.translation
      }
      if (translation.status === 'failed') throw new Error(t(translation.error))
      await new Promise((resolve) => setTimeout(resolve, 350))
    }
    throw new Error(t('Translation timed out'))
  } finally {
    if (threadId) {
      dispatchBackendRpc(backend, 'thread/delete', {
        threadId,
        ...(!isCodexBackend(backend) ? { cwd } : {}),
      }, 15_000).catch((error) => {
        console.warn('Unable to remove the hidden translation session', error)
      })
    }
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

function openSessionResourceSource(occurrence) {
  if (!occurrence?.turnId) return
  transcriptScrollFollower.pause()
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

function connectBackend() {
  if (state.backend === 'opencode') connectOpenCode().catch(showError)
  else connectAppServer()
}

async function switchBackend(backend, { selectedId } = {}) {
  if (!isSupportedBackend(backend) || backend === state.backend) return
  const previousBackend = state.backend
  captureTranscriptViewState()
  state.selectedByBackend[state.backend] = state.selectedId
  cleanupConnections()
  state.backendStates[previousBackend] = { kind: 'idle', label: backendDescriptor(previousBackend).name, caption: 'Connect on demand' }
  rejectPending(new Error('Backend switched'))
  state.backend = backend
  if (selectedId) state.selectedByBackend[backend] = selectedId
  state.selectedId = state.selectedByBackend[backend] || null
  state.threads = state.threadsByBackend[backend]
  state.model = freshThreadModel(backend, state.selectedId)?.model || createCodexViewModel()
  if (state.selectedId) state.model.threadId = state.selectedId
  prepareTranscriptViewForSelection()
  state.backendInfo = state.backendInfos[backend]
  state.ready = false
  applyBackendCopy()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  sessionMap.render()
  persistPreferences()
  await loadBackendInfo()
  connectBackend()
}

function applyBackendCopy() {
  const descriptor = currentBackend()
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

async function connectOpenCode() {
  clearTimeout(state.reconnectTimer)
  cleanupConnections()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  setBackendState('checking', 'Starting OpenCode', 'Server · HTTP/SSE')
  setNativeError(null)
  await loadBackendInfo()
  if (generation !== state.socketGeneration) return
  if (state.backendInfo?.reachable === false || state.backendInfo?.error) {
    const reason = state.backendInfo.error || 'OpenCode Server is not ready'
    setBackendState('error', 'OpenCode unavailable', reason)
    setNativeError(reason)
    return
  }
  state.ready = true
  setBackendState('online', 'OpenCode Server', 'Native structured connection')
  $('#native-connection').textContent = 'Connected'
  loadBackendModels().catch((error) => console.debug('Unable to load OpenCode models', error))
  const events = gatewayEventSource('/opencode/global/event')
  state.eventSource = events
  events.onopen = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('online', 'OpenCode Server', 'Native structured connection')
    $('#native-connection').textContent = 'Connected'
  }
  events.onmessage = (event) => {
    if (generation !== state.socketGeneration) return
    try { handleOpenCodeServerEvent(JSON.parse(event.data)) }
    catch (error) { console.error('Invalid OpenCode SSE event', error, event.data) }
  }
  events.onerror = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('checking', 'Reconnecting to OpenCode', 'SSE event stream')
    $('#native-connection').textContent = 'Reconnecting event stream…'
  }
  await loadThreads().catch((error) => {
    handleThreadCatalogFailure('opencode', generation, error)
  })
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
  for (const timer of openCodeStatusReconcileTimers.values()) clearTimeout(timer)
  openCodeStatusReconcileTimers.clear()
  threadCatalogRetryTimer = null
  threadCatalogRetryAttempt = 0
  threadCatalogErrorMessage = null
  cleanupSocket()
  if (state.eventSource) {
    state.eventSource.close()
    state.eventSource = null
  }
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

function observeCodexTurnLatency(message) {
  const params = message?.params || {}
  const turnId = params.turnId || params.turn?.id
  const threadId = params.threadId || params.thread?.id || state.selectedId
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

function handleAppServerMessage(message) {
  const backend = state.backend
  const descriptor = backendDescriptor(backend)
  if (message.method === 'studio/appServer/status') {
    const status = message.params?.state
    if (status === 'ready') {
      const firstReady = !state.ready
      const previousGeneration = state.appServerGenerations[backend]
      const reconnecting = previousGeneration != null
      const nextGeneration = message.params?.generation ?? previousGeneration
      if (previousGeneration != null && nextGeneration !== previousGeneration) {
        sessionDispatch.clearPrepared(backend)
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
        loadThreads().then(async () => {
          if (reconnecting && state.selectedId) await refreshSelectedThread({ quiet: true })
        }).catch((error) => handleThreadCatalogFailure(backend, state.socketGeneration, error))
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
    if (!state.selectedId) {
      toast(t('The UI missed {count} App Server events', { count: skipped }), 'error')
      return
    }
    setNativeError(`The interface missed ${skipped} App Server events and is resynchronizing the current session…`)
    refreshSelectedThread({ quiet: true }).then((refreshed) => {
      if (!refreshed) return
      setNativeError(null)
      toast(t('Resynchronized the session from {backend}', { backend: descriptor.name }))
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
    sessionManagement.archive.markStale()
    if (message.method === 'thread/deleted') {
      sessionManagement.archive.remove(backend, threadId)
    }
    state.threads = state.threads.filter((thread) => thread.id !== threadId)
    state.threadsByBackend[backend] = state.threads
    if (message.method === 'thread/deleted') {
      delete state.annotationDrafts[`${backend}:${threadId}`]
      delete state.annotationAdditional[`${backend}:${threadId}`]
      delete state.openingMessages[`${backend}:${threadId}`]
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

  updateCodexReplyTime(message)
  observeCodexTurnLatency(message)

  if (message.id != null && message.method) {
    if (message.method === 'item/tool/call' && message.params?.tool === 'update_session_map') {
      sessionMap.handleToolCall(message)
      return
    }
    const targetModel = codexNotificationModel(message)
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

  const targetModel = codexNotificationModel(message)
  if (!targetModel) return
  if (applyCodexNotification(targetModel, message)) {
    markCachedModelValidated(backend, targetModel)
    if (message.method === 'turn/completed') {
      const completedThread = state.threads.find((thread) => thread.id === (message.params?.threadId || targetModel.threadId))
      notifyDesktop(t('Work completed'), threadTitle(completedThread || { name: t('Untitled session') }))
      threadRouter.completeTurn({
        backend,
        turnId: message.params?.turn?.id || message.params?.turnId,
        model: targetModel,
        turn: message.params?.turn,
      }).catch((error) => console.error('Thread Router dispatch failed', error))
    }
    if (targetModel !== state.model) {
      updateThreadStatusFromNotification(message)
      return
    }
    const updateKind = transcriptUpdateKind(message.method)
    if (updateKind === 'stream') queueStreamingItemPatch(message.params)
    else if (updateKind === 'item') replaceCompletedItem(message.params)
    else if (updateKind === 'full') {
      const turnId = message.params?.turnId || message.params?.turn?.id
      const preserveActivity = message.method !== 'turn/completed'
      if (!turnId || !replaceRenderedTurn(turnId, { preserveActivity })) renderTranscript()
    }
    renderComposerState()
    updateSelectedThreadStatus(message)
    if (message.method === 'turn/completed') {
      const threadId = message.params?.threadId || message.params?.thread?.id || state.selectedId
      sessionMap.processInlineUpdate(backend, threadId, targetModel, message.params?.turn?.id).catch((error) => {
        console.warn('Session Map inline update failed', error)
      })
    }
  }
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

function notifyDesktop(title, body = '') {
  if (!state.desktopNotifications || (!document.hidden && document.hasFocus())) return
  if (!('Notification' in window) || Notification.permission !== 'granted') return
  try {
    const notification = new Notification(String(title || 'Codex Thread Studio'), {
      body: String(body || '').slice(0, 240),
      tag: `codex-thread-studio:${state.backend}:${state.selectedId || 'workspace'}`,
    })
    notification.onclick = () => {
      window.focus()
      notification.close()
    }
  } catch (error) {
    console.warn('Desktop notification failed', error)
  }
}

function handleOpenCodeServerEvent(event) {
  const payload = event?.payload || event
  if (!payload?.type || payload.type === 'sync' || payload.type === 'server.heartbeat') return
  const eventThreadId = openCodeEventThreadId(payload)
  const eventThread = payload.properties?.info || { id: eventThreadId }
  if (hiddenUtilityThread('opencode', eventThread)) {
    if (eventThreadId) markUtilityThreadHidden('opencode', eventThreadId)
    return
  }
  if (payload.type === 'server.connected') {
    scheduleOpenCodeListRefresh()
    return
  }
  if (payload.type.startsWith('session.')) scheduleOpenCodeListRefresh()
  if (payload.type === 'session.deleted') {
    const deletedId = payload.properties?.info?.id || payload.properties?.sessionID
    const deletedKey = sessionRefKey('opencode', deletedId)
    delete state.openingMessages[deletedKey]
    discardComposerSessionState('opencode', deletedId)
    if (threadRouter.removeSession('opencode', deletedId)) persistPreferences()
    if (deletedId && state.selectedId === deletedId) {
      state.selectedId = null
      state.selectedByBackend.opencode = null
      state.model = createCodexViewModel()
      persistPreferences()
      renderWorkspace()
    }
    invalidateThreadModel('opencode', deletedId)
    return
  }
  if (eventThreadId && openCodeCompletionSignal(payload)) scheduleOpenCodeStatusReconciliation(eventThreadId)
  updateOpenCodeReplyTime(payload, eventThreadId)
  const cached = eventThreadId && state.threadModels.get(threadCatalogKey('opencode', eventThreadId))
  const targetModel = eventThreadId === state.selectedId
    ? state.model
    : cached?.model
  if (!targetModel) return
  const update = applyOpenCodeEvent(targetModel, event, eventThreadId)
  if (!update.handled) return
  markCachedModelValidated('opencode', targetModel)
  if (targetModel !== state.model) return
  if (update.kind === 'stream') queueStreamingItemPatch({ turnId: update.turnId, itemId: update.itemId })
  else if (update.kind === 'metadata') {
    renderComposerState()
  } else if (!update.turnId || !replaceRenderedTurn(update.turnId, { preserveActivity: payload.type !== 'session.idle' })) renderTranscript()
  renderComposerState()
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
    if (state.backend !== 'opencode' || !state.ready) return
    const thread = state.threadsByBackend.opencode.find((candidate) => candidate.id === threadId)
    if (!thread) return
    try {
      const statuses = await openCodeFetch(withDirectory('/session/status', thread.cwd), { timeoutMs: 10_000 })
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

function scheduleOpenCodeListRefresh() {
  clearTimeout(openCodeListRefreshTimer)
  openCodeListRefreshTimer = setTimeout(() => refreshOpenCodeThreadList().catch(console.error), 180)
}

async function refreshOpenCodeThreadList() {
  if (state.backend !== 'opencode' || !state.ready) return
  const result = await rpc('thread/list', { limit: 100 })
  setActiveThreads(Array.isArray(result?.data) ? result.data : [])
  renderThreadList()
  renderWorkspace()
  if (state.selectedId && !freshThreadModel('opencode', state.selectedId)) {
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

async function openCodeFetch(path, { method = 'GET', body, timeoutMs = 30_000, allowInactive = false } = {}) {
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
    if (response.status === 204) return null
    const text = await response.text()
    let value = null
    try { value = text ? JSON.parse(text) : null } catch { value = text }
    if (!response.ok) throw new Error(value?.error?.message || value?.message || `${method} ${path} failed: HTTP ${response.status}`)
    return value
  } catch (error) {
    if (error.name === 'AbortError') throw new Error(t('{method} {path} request timed out', { method, path }))
    throw error
  } finally { clearTimeout(timer) }
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
  codexCatalogRecoveryStarted.add(backend)
  queueMicrotask(async () => {
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

function installBackendCatalog(backend, threads) {
  const visible = (threads || []).filter((thread) => !hiddenUtilityThread(backend, thread))
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
    const messages = await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/message`, session.directory, 'limit=500'), fetchOptions)
    const statuses = await openCodeFetch(withDirectory('/session/status', session.directory), fetchOptions).catch(() => ({}))
    return { thread: openCodeThreadFromHistory(session, messages, statuses?.[session.id] || 'idle') }
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
      return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/command`, directory), {
        method: 'POST',
        body: { command: skill.name, arguments: text, agent: 'build' },
        timeoutMs: Math.max(timeoutMs, 300_000),
      })
    }
    const imageParts = (params.input || []).map(openCodeImagePart).filter(Boolean)
    const parts = [
      ...(text ? [{ type: 'text', text }] : []),
      ...(params.input || []).filter((item) => item?.type === 'file').map(openCodeFilePart),
      ...imageParts,
    ]
    await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/prompt_async`, directory), {
      method: 'POST',
      body: {
        parts,
        ...(params.clientUserMessageId ? { messageID: openCodeMessageId(params.clientUserMessageId) } : {}),
        ...(model ? { model } : {}),
        ...(params.developerInstructions ? { system: params.developerInstructions } : {}),
        ...(params.outputSchema ? { format: { type: 'json_schema', schema: params.outputSchema, retryCount: 2 } } : {}),
      },
      timeoutMs,
      allowInactive,
    })
    return null
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

async function loadThreads() {
  const result = await rpc('thread/list', catalogListParams(backendDescriptor(state.backend).kind, { limit: 100 }))
  setActiveThreads(Array.isArray(result?.data) ? result.data : [])
  markThreadCatalogLoaded()
  if (state.backend === state.router.controllerBackend) {
    await threadRouter.ensureManagedSession(state.backend).catch(showError)
  }
  renderThreadList()
  scheduleCodexCatalogRecovery(state.backend)
  refreshInactiveCatalog()
  if (sessionManagement.archive.isOpen()) {
    renderWorkspace()
    return
  }
  const visibleThreads = sidebarThreadsForBackend(state.backend).filter((thread) => !isSessionDirectoryHidden(thread.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore))
  const preferred = state.selectedId
  const recent = [...visibleThreads].sort((left, right) => threadUpdatedAt(right) - threadUpdatedAt(left))[0]
  const nextId = visibleThreads.some((thread) => thread.id === preferred) ? preferred : recent?.id
  if (nextId) await selectThread(nextId, { force: true }).catch(showError)
  else renderWorkspace()
}

function setActiveThreads(threads) {
  const visible = (threads || []).filter((thread) => !hiddenUtilityThread(state.backend, thread))
  state.threads = visible
  state.threadsByBackend[state.backend] = visible
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
  const counts = catalogCountsWithAttention(sidebarThreadCatalogs(), state.attentionThreads, state.hiddenSessionDirectories, state.sessionDirectoryIgnore)
  $('#count-all').textContent = counts.all
  $('#count-active').textContent = counts.active
  $('#count-attention').textContent = counts.attention
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
    const tag = descriptor.tag
    const router = backend === state.router.controllerBackend && isRouterSession(state.router, backend, thread.id)
    return `<button class="thread-row${active ? ' active' : ''}" data-thread-id="${escapeHtml(thread.id)}" data-backend="${backend}">
      <span class="status-dot ${escapeHtml(status)}"></span>
      <span class="thread-copy"><strong>${escapeHtml(threadTitle(thread))}</strong><small data-no-i18n title="${escapeHtml(thread.cwd || t('Project directory not recorded'))}">${escapeHtml(thread.cwd || t('Project directory not recorded'))}</small></span>
      <span class="thread-tags">${router ? '<span class="backend-tag router" title="Thread Router">RT</span>' : ''}<span class="backend-tag ${backend}" title="${descriptor.name}">${tag}</span></span>
    </button>`
  }
  if (state.filter === 'attention') {
    list.innerHTML = entries.map(renderRow).join('')
  } else {
    list.innerHTML = groupCatalogEntries(entries).map(({ cwd, name, entries: groupEntries }) => {
      const label = name || t('Other sessions')
      const collapsed = state.collapsedThreadGroups.has(cwd)
      return `<section class="thread-group${collapsed ? ' collapsed' : ''}" data-group-path="${escapeHtml(cwd)}">
        <button class="thread-group-heading" type="button" data-no-i18n title="${escapeHtml(cwd || t('Project directory not recorded'))}">
          <span class="twisty">▼</span><strong>${escapeHtml(label)}</strong><span>${groupEntries.length}</span>
        </button>
        <div class="thread-group-sessions">${groupEntries.map(renderRow).join('')}</div>
      </section>`
    }).join('')
  }
  list.querySelectorAll('.thread-group-heading').forEach((button) => button.addEventListener('click', () => {
    const path = button.closest('.thread-group').dataset.groupPath
    if (state.collapsedThreadGroups.has(path)) state.collapsedThreadGroups.delete(path)
    else state.collapsedThreadGroups.add(path)
    renderThreadList()
  }))
  list.querySelectorAll('.thread-row').forEach((row) => row.addEventListener('click', () =>
    selectThread(row.dataset.threadId, { backend: row.dataset.backend }),
  ))
}

async function selectThread(id, { force = false, backend = state.backend } = {}) {
  if (backend !== state.backend) {
    await switchBackend(backend, { selectedId: id })
    await waitFor(() => state.backend === backend && state.ready, 15_000)
    await waitFor(() => state.threads.some((thread) => thread.id === id), 15_000)
    if (state.selectedId === id && freshThreadModel(backend, id)) return
    return selectThread(id, { force: true, backend })
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
    if (state.selectedId === id) sessionMap.setSyncState('error', error.message)
  })
  const cached = freshThreadModel(state.backend, id)
  state.model = cached?.model || createCodexViewModel()
  state.model.threadId = id
  prepareTranscriptViewForSelection()
  persistPreferences()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  if (cached) {
    $('#native-connection').textContent = t('Restored from cache')
    await mapLoad
    await activateSelectedEnvironment().catch((error) => reportClientError(error))
    sessionMap.maybeBootstrap(key, state.model)
    return
  }
  await resumeThread(id)
  await mapLoad
  await activateSelectedEnvironment().catch((error) => reportClientError(error))
  sessionMap.maybeBootstrap(key, state.model)
}

function markThreadLoaded(backend, id) {
  if (addLoadedThread(state.attentionThreads, backend, id)) renderThreadList()
}

function updateCodexReplyTime(message, model = null) {
  if (message.method !== 'turn/completed') return
  const backend = isCodexBackend(state.backend) ? state.backend : 'codex'
  const threadId = message.params?.threadId
    || message.params?.thread?.id
    || message.params?.turn?.threadId
    || threadIdForCachedModel(backend, model)
    || (state.backend === backend ? state.selectedId : null)
  if (!threadId) return
  updateLoadedThreadTimestamp(backend, threadId)
}

function updateOpenCodeReplyTime(payload, threadId) {
  if (!threadId || payload.type !== 'session.idle') return
  updateLoadedThreadTimestamp('opencode', threadId)
}

function updateLoadedThreadTimestamp(backend, id) {
  if (updateLoadedCatalogTimestamp(
    state.threadsByBackend,
    state.attentionThreads,
    backend,
    id,
    Date.now(),
  ) && state.filter === 'attention') renderThreadList()
}

function threadUpdatedAt(thread) {
  return catalogTimestamp(thread?.updatedAt || thread?.updated_at || thread?.createdAt)
}

async function resumeThread(id) {
  const key = threadCatalogKey(state.backend, id)
  if (state.threadLoads.has(key)) return state.threadLoads.get(key)
  const load = resumeThreadUncached(id).finally(() => state.threadLoads.delete(key))
  state.threadLoads.set(key, load)
  return load
}

async function resumeThreadUncached(id) {
  setNativeError(null)
  $('#native-connection').textContent = 'Resuming session…'
  try {
    const result = await rpc('thread/resume', { threadId: id })
    sessionDispatch.markPrepared({ backend: state.backend, id })
    if (state.selectedId !== id) return
    hydrateCodexThread(state.model, result.thread)
    if (state.backend === 'opencode') hydrateOpenCodeModelMetadata(result.thread)
    mergeThreadMetadata(result.thread)
    cacheThreadModel(state.backend, id)
    $('#native-connection').textContent = 'Connected'
    renderWorkspace()
    renderTranscript()
  } catch (error) {
    if (state.selectedId !== id) return
    state.model.error = error.message
    state.model.status = 'failed'
    setNativeError(t('Unable to resume this {backend} session: {message}', { backend: currentBackend().name, message: error.message }))
    renderWorkspace()
  }
}

async function refreshSelectedThread({ quiet = false } = {}) {
  if (!state.selectedId) return false
  const threadId = state.selectedId
  try {
    const result = await rpc('thread/read', { threadId, includeTurns: true })
    if (state.selectedId !== threadId) return false
    hydrateCodexThread(state.model, result.thread)
    if (state.backend === 'opencode') hydrateOpenCodeModelMetadata(result.thread)
    mergeThreadMetadata(result.thread)
    cacheThreadModel(state.backend, threadId)
    renderWorkspace()
    renderTranscript()
    if (!quiet) toast('Session refreshed')
    return true
  } catch (error) {
    if (quiet) setNativeError(t('Unable to resynchronize the current {backend} session: {message}', { backend: currentBackend().name, message: error.message }))
    else showError(error)
    return false
  }
}

function freshThreadModel(backend, id) {
  const cached = state.threadModels.get(threadCatalogKey(backend, id))
  if (!cached) return null
  const thread = state.threadsByBackend[backend].find((candidate) => candidate.id === id)
  const updatedAt = catalogTimestamp(thread?.updatedAt || thread?.updated_at || thread?.createdAt)
  return !updatedAt || updatedAt <= cached.validatedAt ? cached : null
}

function cacheThreadModel(backend = state.backend, id = state.selectedId, model = state.model) {
  if (!id || !model || model.threadId !== id) return
  state.threadModels.set(threadCatalogKey(backend, id), {
    model,
    validatedAt: Date.now(),
  })
  markThreadLoaded(backend, id)
}

function invalidateThreadModel(backend, id) {
  if (!id) return
  const key = threadCatalogKey(backend, id)
  state.threadModels.delete(key)
  transcriptPresentationCache.invalidateThread(key)
  if (state.attentionThreads.delete(key)) persistPreferences()
}

function markCachedModelValidated(backend, model) {
  for (const [key, cached] of state.threadModels) {
    if (key.startsWith(`${backend}:`) && cached.model === model) {
      cached.validatedAt = Date.now()
      return
    }
  }
}

function threadIdForCachedModel(backend, model) {
  if (state.backend === backend && state.model === model) return state.selectedId
  for (const [key, cached] of state.threadModels) {
    if (key.startsWith(`${backend}:`) && cached.model === model) return key.slice(backend.length + 1)
  }
  return null
}

function codexNotificationModel(message) {
  const backend = isCodexBackend(state.backend) ? state.backend : 'codex'
  const params = message.params || {}
  const explicitId = params.threadId || params.thread?.id || params.turn?.threadId
  if (explicitId && state.hiddenCodexThreads.has(sessionRefKey(backend, explicitId))) return null
  if (explicitId) {
    if (state.backend === backend && state.selectedId === explicitId) return state.model
    return state.threadModels.get(threadCatalogKey(backend, explicitId))?.model || null
  }
  const turnId = params.turnId || params.turn?.id
  if (turnId && state.hiddenCodexTurns.has(routerRuntimeKey(backend, turnId))) return null
  if (turnId) {
    for (const [key, cached] of state.threadModels) {
      if (!key.startsWith(`${backend}:`)) continue
      if (cached.model.activeTurnId === turnId || cached.model.turns.some((turn) => turn.id === turnId)) return cached.model
    }
  }
  return state.model
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

function updateThreadStatusFromNotification(message) {
  if (message.method !== 'thread/status/changed') return
  const threadId = message.params?.threadId
  const thread = state.threadsByBackend[state.backend]?.find((candidate) => candidate.id === threadId)
  if (thread) thread.status = message.params.status
  renderThreadList()
}

function hydrateOpenCodeModelMetadata(thread) {
  state.model.messageTurns = { ...(thread?.messageTurns || {}) }
  state.model.messageRoles = { ...(thread?.messageRoles || {}) }
  state.model.status = thread?.status || state.model.status
  state.model.activeTurnId = state.model.status === 'running' ? state.model.turns.at(-1)?.id || null : null
}

function mergeThreadMetadata(incoming) {
  if (!incoming?.id) return
  const index = state.threads.findIndex((thread) => thread.id === incoming.id)
  if (index >= 0) state.threads[index] = {
    ...mergeCatalogMetadata(backendDescriptor(state.backend).kind, state.threads[index], incoming),
    turns: undefined,
  }
  else state.threads.unshift({ ...incoming, turns: undefined })
  state.threadsByBackend[state.backend] = state.threads
  renderThreadList()
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
  const text = state.model.turns.map((turn) => questionForTurn(turn).trim()).find(Boolean)
  if (!text) return
  const normalized = truncateUtf8(text, 16 * 1024)
  state.openingMessages[key] = {
    ...(state.openingMessages[key] || {}),
    text: normalized,
    source: 'history',
    capturedAt: new Date().toISOString(),
    truncated: normalized !== text,
  }
  persistPreferences()
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
    await persistPreferences()
    $('#thread-info-dialog').close()
    toast(t('Session information saved'))
  } catch (error) {
    if (hadExisting) state.openingMessages[key] = existing
    else delete state.openingMessages[key]
    showError(error)
  }
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
    applyCodexNotification(state.model, { method: 'turn/started', params: { threadId: ref.id, turn: result.turn } })
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
  for (const id of ['rename-thread', 'fork-thread', 'archive-thread', 'refresh-thread', 'project-environment-action', 'session-map-action', 'router-settings-action']) {
    $(`#${id}`).classList.toggle('hidden', archived)
  }
  $('.thread-action-menu .menu-separator')?.classList.toggle('hidden', archived)
  workspaceTools.sync(thread)
  sessionResources.sync()
  if (!thread) {
    sessionManagement.search.close()
    sessionMap.render()
    return
  }
  $('#thread-title').textContent = threadTitle(thread)
  $('#thread-path').textContent = thread.cwd || thread.id
  $('#archive-thread').disabled = archived || state.backend === 'opencode'
  $('#archive-thread').title = t(state.backend === 'opencode' ? 'The OpenCode backend does not support archiving yet' : 'Archive session')
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

function transcriptReadingTurnId(container = $('#transcript')) {
  const containerRect = container.getBoundingClientRect()
  const marker = containerRect.top + Math.min(container.clientHeight * 0.28, 160)
  const positions = [...container.querySelectorAll('.turn[data-turn-id]')]
    .map((turn) => ({ id: turn.dataset.turnId, top: turn.getBoundingClientRect().top }))
  return activeTurnAtMarker(positions, marker, distanceFromBottom(container) < 8) || ''
}

function captureTranscriptViewState() {
  if (!state.selectedId) return
  const key = presentationThreadKey()
  if (pendingTranscriptViewRestore?.key === key) return
  const container = $('#transcript')
  const entry = currentPresentationEntry()
  const anchor = transcriptReadingAnchor(container)
  const readingTurnId = transcriptReadingTurnId(container)
  const bottomDistance = distanceFromBottom(container)
  transcriptPresentationCache.setScrollState(key, {
    scrollTop: container.scrollTop,
    anchorTurnId: anchor.turnId,
    anchorOffset: anchor.offset,
    followOnReturn: shouldFollowLatestOnReturn({
      orderedTurnIds: entry.orderedIds,
      readingTurnId,
      bottomDistance,
    }),
  })
}

function prepareTranscriptViewForSelection() {
  if (!state.selectedId) {
    pendingTranscriptViewRestore = null
    transcriptScrollFollower.reset()
    return
  }
  const key = presentationThreadKey()
  const saved = transcriptPresentationCache.scrollState(key)
  if (saved && !saved.followOnReturn) {
    pendingTranscriptViewRestore = { key, ...saved }
    transcriptScrollFollower.pause()
  } else {
    pendingTranscriptViewRestore = null
    transcriptScrollFollower.reset()
  }
}

function restoreTranscriptView(container = $('#transcript')) {
  if (!state.selectedId || transcriptScrollFollower.following) return false
  const key = presentationThreadKey()
  const saved = pendingTranscriptViewRestore?.key === key
    ? pendingTranscriptViewRestore
    : transcriptPresentationCache.scrollState(key)
  if (!saved) return false
  const anchor = saved.anchorTurnId
    ? [...container.querySelectorAll('.turn[data-turn-id]')]
      .find((turn) => turn.dataset.turnId === saved.anchorTurnId)
    : null
  if (anchor) {
    const containerRect = container.getBoundingClientRect()
    container.scrollTop += anchor.getBoundingClientRect().top - containerRect.top - Number(saved.anchorOffset || 0)
    if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
    return true
  }
  if (!saved.anchorTurnId && Number.isFinite(saved.scrollTop)) {
    container.scrollTop = saved.scrollTop
    if (pendingTranscriptViewRestore?.key === key) pendingTranscriptViewRestore = null
    return true
  }
  return false
}

function renderTranscript({ preserveScroll = false, previousHeight = 0, previousTop = 0 } = {}) {
  if (!state.selectedId) return
  resetStreamingPatches()
  const container = $('#transcript')
  const turns = state.model.turns || []
  const entry = currentPresentationEntry()
  const visibleIds = entry.orderedIds.slice(entry.visibleStart)
  const turnById = new Map(turns.map((turn) => [String(turn.id || ''), turn]))
  const older = entry.visibleStart > 0
    ? `<button class="load-earlier-turns" type="button" data-load-earlier>${t('{count} earlier turns', { count: entry.visibleStart })}</button>`
    : ''
  container.innerHTML = older + visibleIds.map((id) => {
    const turn = turnById.get(id)
    if (!turn) return ''
    const index = entry.orderedIds.indexOf(id)
    if (isRouterThread()) return threadRouter.renderTurn(turn, index)
    return renderTurn(entry.turns.get(id)?.presentation, index)
  }).join('') + renderApprovals()
  observeTranscriptContent()
  bindApprovalButtons()
  bindActivityDetails()
  reviewNotes.renderCommentMarkers()
  renderTurnNavigator()
  if (preserveScroll) {
    container.scrollTop = previousTop + Math.max(0, container.scrollHeight - previousHeight)
    captureTranscriptViewState()
  } else if (!restoreTranscriptView(container)) followTranscriptOutput()
  captureOpeningMessage()
  sessionResources.sync({ rebuild: true })
}

function handleTranscriptScroll() {
  const transcript = $('#transcript')
  transcriptScrollFollower.handleScroll(transcript)
  captureTranscriptViewState()
  scheduleTurnNavigatorSync()
}

function followTranscriptOutput() {
  if (!transcriptScrollFollower.following) return
  const transcript = $('#transcript')
  transcript.scrollTop = transcript.scrollHeight
  scheduleTurnNavigatorSync()
}

function observeTranscriptContent() {
  transcriptContentObserver.observe($('#transcript'))
}

function handleTranscriptContentResize() {
  if (!restoreTranscriptView()) followTranscriptOutput()
  scheduleTurnNavigatorSync()
}

function renderTurnNavigator() {
  const navigator = $('#turn-navigator')
  const list = $('#turn-navigator-list')
  const turns = navigableTurns(state.model.turns)
  if (turns.length < 2) {
    navigator.classList.add('hidden')
    list.innerHTML = ''
    return
  }

  list.innerHTML = turns.map((turn, index) => {
    const label = turnNavigationLabel(turn, index)
    const title = turnPromptPreview(turn) || t('User input {index}', { index: index + 1 })
    return `<button class="turn-nav-item" type="button" data-turn-nav-id="${escapeHtml(turn.id || '')}" aria-label="${escapeHtml(label)}"><span class="turn-nav-title">${escapeHtml(title)}</span><span class="turn-nav-indicator" aria-hidden="true"><i></i></span></button>`
  }).join('')
  navigator.classList.remove('hidden')
  scheduleTurnNavigatorSync()
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
  const navigableIds = new Set(navigableTurns(state.model.turns).map((turn) => String(turn.id || '')))
  const positions = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .filter((element) => navigableIds.has(element.dataset.turnId))
    .map((element) => ({ id: element.dataset.turnId, top: element.getBoundingClientRect().top }))
  const atBottom = transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 8
  setActiveTurnNavigator(activeTurnAtMarker(positions, marker, atBottom))
}

function setActiveTurnNavigator(turnId) {
  const list = $('#turn-navigator-list')
  let activeButton = null
  list.querySelectorAll('[data-turn-nav-id]').forEach((button) => {
    const active = button.dataset.turnNavId === String(turnId || '')
    button.classList.toggle('active', active)
    if (active) {
      button.setAttribute('aria-current', 'true')
      activeButton = button
    } else {
      button.removeAttribute('aria-current')
    }
  })
  if (!activeButton) return
  if (activeButton.offsetTop < list.scrollTop) list.scrollTop = activeButton.offsetTop
  else if (activeButton.offsetTop + activeButton.offsetHeight > list.scrollTop + list.clientHeight) {
    list.scrollTop = activeButton.offsetTop + activeButton.offsetHeight - list.clientHeight
  }
}

function handleTurnNavigatorClick(event) {
  const button = event.target.closest('[data-turn-nav-id]')
  if (!button) return
  const transcript = $('#transcript')
  let target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .find((turn) => turn.dataset.turnId === button.dataset.turnNavId)
  if (!target) {
    transcriptPresentationCache.showTurn(presentationThreadKey(), state.model, button.dataset.turnNavId)
    renderTranscript()
    target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
      .find((turn) => turn.dataset.turnId === button.dataset.turnNavId)
  }
  if (!target) return
  transcriptScrollFollower.pause()
  const top = transcript.scrollTop + target.getBoundingClientRect().top - transcript.getBoundingClientRect().top - 16
  setActiveTurnNavigator(button.dataset.turnNavId)
  transcript.scrollTo({ top: Math.max(0, top), behavior: 'smooth' })
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
  return [...$('#transcript').querySelectorAll('[data-item-id]')]
    .find((element) => element.dataset.turnId === String(turnId || '') && element.dataset.itemId === String(itemId || '')) || null
}

function patchStreamingItem(turnId, itemId) {
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
  sessionResources.sync({ rebuild: true })
}

function renderedActivity(turnId, activityId = '') {
  return [...$('#transcript').querySelectorAll('.work-activity[data-turn-id]')]
    .find((element) => element.dataset.turnId === String(turnId || '')
      && (!activityId || element.dataset.activityId === String(activityId))) || null
}

function replaceRenderedTurn(turnId, { preserveActivity = true } = {}) {
  const section = [...$('#transcript').querySelectorAll('.turn[data-turn-id]')]
    .find((element) => element.dataset.turnId === String(turnId || ''))
  if (!section) return false
  const openActivityIds = preserveActivity
    ? [...section.querySelectorAll('.work-activity[open]')].map((activity) => activity.dataset.activityId)
    : []
  const entry = transcriptPresentationCache.updateTurn(presentationThreadKey(), state.model, turnId)
  const presentation = entry.turns.get(String(turnId || ''))?.presentation
  if (!presentation) return false
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
  renderTurnNavigator()
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
  if (block.type === 'error') return `<div class="turn-error" role="alert"><strong>${t('Execution failed')}</strong><span>${escapeHtml(block.message)}</span></div>`
  return ''
}

function conversationTrackIcon(kind) {
  if (kind === 'working') return '<span class="track-icon activity-spinner"></span>'
  const shapes = {
    question: '<path d="m6.2 4.7 4.5 4.3-4.5 4.3"></path>',
    response: '<circle cx="9" cy="9" r="4.6"></circle>',
    completed: '<path d="m4.5 9.1 3 3.1 6-6.2"></path>',
    failed: '<path d="M9 4.2v6.2"></path><path d="M9 13.5v.1"></path>',
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
    fontFamily: state.typography.uiFontFamily,
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
    await openArtifact({ root: selectedThread()?.cwd, path: file.path }, { returnTool: state.activeRightWorkspace === 'resources' ? 'resources' : '' })
    if (file.line) jumpArtifactToLine(file.line, file.column)
    return
  }
  const earlier = event.target.closest('[data-load-earlier]')
  if (earlier) {
    const transcript = $('#transcript')
    transcriptScrollFollower.pause()
    const previousHeight = transcript.scrollHeight
    const previousTop = transcript.scrollTop
    transcriptPresentationCache.showEarlier(presentationThreadKey(), state.model, 20)
    renderTranscript({ preserveScroll: true, previousHeight, previousTop })
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
    return `<pre><code>$ ${escapeHtml(command)}${item.aggregatedOutput ? `\n\n${escapeHtml(item.aggregatedOutput)}` : ''}</code></pre>`
  }
  if (entry.kind === 'reasoning') {
    const content = arrayText(item.summary) || arrayText(item.content) || ''
    return `<div class="markdown-body compact-markdown">${renderMarkdown(content)}</div>`
  }
  if (entry.kind === 'progress') return `<div class="markdown-body compact-markdown">${renderMarkdown(sessionMapVisibleText(item.text || ''))}</div>`
  if (entry.kind === 'change') {
    const changes = (item.changes || []).map((change) => `${change.kind || 'update'} ${change.path || ''}\n${change.diff || ''}`).join('\n\n')
    return `<pre><code>${escapeHtml(changes)}</code></pre>`
  }
  if (entry.kind === 'plan') {
    const rows = (item.plan || []).map((step) => `${step.status || 'pending'}  ${step.step || ''}`).join('\n')
    return `<pre><code>${escapeHtml([item.explanation || '', rows].filter(Boolean).join('\n\n'))}</code></pre>`
  }
  return `<pre><code>${escapeHtml(valueText(item))}</code></pre>`
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
  $$('.approval-card').forEach((card) => {
    card.querySelector('.approval-decline').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'decline'))
    card.querySelector('.approval-session').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'acceptForSession'))
    card.querySelector('.approval-accept').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'accept'))
  })
  $$('.interaction-card').forEach((card) => {
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
  if (!models.length) {
    $('#command-content').innerHTML = '<div class="command-empty">No models are available.</div>'
    return
  }
  const currentEffort = currentTurnOptions().effort
  $('#command-content').innerHTML = `<div class="command-list">${models.map((model) => {
    const efforts = model.supportedReasoningEfforts || []
    const selectedEffort = efforts.some((entry) => entry.reasoningEffort === currentEffort)
      ? currentEffort
      : model.defaultReasoningEffort
    const effortOptions = efforts.map((entry) => `<option value="${escapeHtml(entry.reasoningEffort)}"${entry.reasoningEffort === selectedEffort ? ' selected' : ''}>${escapeHtml(entry.reasoningEffort)}</option>`).join('')
    return `<div class="command-card"><strong>${escapeHtml(model.displayName || model.model || model.id)}</strong><small>${escapeHtml(model.model || model.id)}${model.isDefault ? t(' · default') : ''}</small>${effortOptions ? `<select aria-label="${t('Reasoning effort')}">${effortOptions}</select>` : '<span></span>'}<button class="subtle-button" type="button" data-model="${escapeHtml(model.model || model.id)}">${t('Use')}</button></div>`
  }).join('')}</div>`
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-model]')
    if (!button) return
    const options = currentTurnOptions()
    options.model = button.dataset.model
    const effort = button.closest('.command-card')?.querySelector('select')?.value
    if (effort) options.effort = effort
    else delete options.effort
    $('#command-dialog').close()
    renderComposerState()
    toast(t('Selected model {model}{effort}', { model: options.model, effort: effort ? ` · ${effort}` : '' }))
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
  transcriptScrollFollower.reset()
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

async function copyLatestAgentResponse() {
  const items = state.model.turns.flatMap((turn) => turn.items || []).reverse()
  const message = items.find((item) => (item.type === 'agentMessage' || item.type === 'plan') && item.text)
  if (!message) throw new Error(t('This session has no {backend} response to copy.', { backend: currentBackend().name }))
  await navigator.clipboard.writeText(message.type === 'agentMessage' ? sessionMapVisibleText(message.text) : message.text)
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
  $('#composer-form').classList.toggle('shell-mode', shellMode)
  renderComposerImages()
  $('#interrupt-turn').classList.toggle('hidden', !active)
  $('#archive-thread').disabled = active || state.backend === 'opencode'
  $('#delete-thread').disabled = active
  $('#send-message').textContent = shellMode ? t('Run') : isRouterThread() ? t('Route') : active && isCodexBackend(state.backend) ? 'Steer' : 'Send'
  $('#send-message').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode') || (shellMode && (active || !shellCommand))
  $('#composer-add-image').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode')
  reviewNotes.renderComposerContext()
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
    transcriptScrollFollower.reset()
    try {
      await rpc('thread/shellCommand', { threadId: state.selectedId, command: shellCommand }, 120_000)
      setComposerDraftValue(initialStateKey, '')
      hideComposerMenu()
      renderComposerState()
      toast(t('Shell command sent to {backend}', { backend: currentBackend().name }))
    } catch (error) { showError(error) }
    finally { button.disabled = false }
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
    transcriptScrollFollower.reset()
    try {
      await threadRouter.startTurn(text, imageInputs)
      setComposerDraftValue(stateKey, '')
      state.pendingImages[stateKey] = []
      hideComposerMenu()
      renderComposerState()
    } catch (error) { showError(error) }
    finally { button.disabled = false }
    return
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
  transcriptScrollFollower.reset()
  let optimisticTurnId = null
  let latencyTrace = null
  let composerCleared = false
  try {
    if (state.model.activeTurnId) {
      await rpc('turn/steer', {
        threadId,
        expectedTurnId: state.model.activeTurnId,
        clientUserMessageId: randomId(),
        input: turnInput,
      })
      toast('Message added to the current turn')
    } else {
      const clientUserMessageId = randomId()
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
      await sessionMap.prepareTurn().catch((error) => {
        console.warn('Unable to attach Session Map context', error)
        sessionMap.setSyncState('error', error.message)
      })
      const result = await rpc('turn/start', turnStartParams(backendDescriptor(backend).kind, threadForRef({ backend, id: threadId }), {
        threadId,
        clientUserMessageId,
        input: turnInput,
        ...turnOptions,
      }))
      if (result?.turn) {
        if (isCodexBackend(backend) && optimisticTurnId) {
          reconcileOptimisticCodexTurn(targetModel, optimisticTurnId, result.turn)
          bindTurnLatencyTrace(latencyTrace, result.turn.id)
          markTurnLatency(latencyTrace, 'turn_start_ack')
        } else {
          applyCodexNotification(targetModel, { method: 'turn/started', params: { turn: result.turn } })
        }
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
  finally { button.disabled = false }
}

function isRouterThread(threadId = state.selectedId, backend = state.backend) {
  return threadRouter.isThread(threadId, backend)
}

async function ensureSessionModel(ref) {
  const key = threadCatalogKey(ref.backend, ref.id)
  const result = await sessionDispatch.read(ref)
  const model = state.backend === ref.backend && state.selectedId === ref.id
    ? state.model
    : state.threadModels.get(key)?.model || createCodexViewModel()
  hydrateCodexThread(model, result.thread)
  if (ref.backend === 'opencode') {
    model.messageTurns = { ...(result.thread?.messageTurns || {}) }
    model.messageRoles = { ...(result.thread?.messageRoles || {}) }
    model.status = result.thread?.status || model.status
    model.activeTurnId = model.status === 'running' ? model.turns.at(-1)?.id || null : null
  }
  mergeThreadIntoCatalog(ref.backend, result.thread)
  cacheThreadModel(ref.backend, ref.id, model)
  return model
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

async function interruptTurn() {
  if (!state.selectedId || !state.model.activeTurnId) return
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
    if (name) await rpc('thread/name/set', { threadId: result.thread.id, name })
    closeNewThreadDialog()
    $('#new-thread-form').reset()
    await loadThreads()
    await selectThread(result.thread.id, { force: true, backend })
    toast(t('{backend} session created', { backend: currentBackend().name }))
  } catch (error) {
    errorBox.textContent = error.message
    errorBox.classList.remove('hidden')
  } finally { button.disabled = false }
}

async function forkThread(lastTurnId = null, trigger = null) {
  const sourceThreadId = state.selectedId
  const sourceBackend = state.backend
  if (!sourceThreadId || isArchivedPreview()) return
  if (trigger) {
    trigger.disabled = true
    trigger.classList.add('busy')
  }
  try {
    const result = await rpc('thread/fork', threadForkParams(sourceThreadId, lastTurnId))
    await loadThreads()
    await selectThread(result.thread.id, { force: true, backend: sourceBackend })
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
  try {
    await rpc('thread/archive', { threadId })
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
    invalidateThreadModel(state.backend, threadId)
    delete state.annotationDrafts[`${state.backend}:${threadId}`]
    delete state.annotationAdditional[`${state.backend}:${threadId}`]
    delete state.openingMessages[`${state.backend}:${threadId}`]
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
  let saved = {}
  try {
    const response = await gatewayFetch('/studio/preferences', { cache: 'no-store' })
    if (response.ok) saved = await response.json()
  } catch (error) { console.warn('Unable to load preferences', error) }
  state.language = normalizeLanguage(saved.language)
  state.theme = saved.theme === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth(saved.contentWidth)
  state.hiddenSessionDirectories = normalizeHiddenSessionDirectories(saved.hiddenSessionDirectories)
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
  state.desktopNotifications = Boolean(saved.desktopNotifications)
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
  state.backend = isSupportedBackend(saved.selectedBackend) ? saved.selectedBackend : 'codex'
  state.selectedByBackend = emptyBackendSelections()
  for (const [backend, id] of Object.entries(saved.selectedThreads || {})) {
    if (isSupportedBackend(backend) && typeof id === 'string' && id) state.selectedByBackend[backend] = id
  }
  if (!state.selectedByBackend.codex && typeof saved.selectedThread === 'string') {
    state.selectedByBackend.codex = saved.selectedThread
  }
  state.attentionThreads = new Set()
  state.selectedId = state.selectedByBackend[state.backend]
  state.annotationDrafts = normalizeAnnotationDrafts(saved.annotationDrafts)
  state.annotationAdditional = normalizeAdditional(saved.annotationAdditional)
  let initialLocale = resolveLanguage(state.language)
  const templateMigration = migrateLocalizedTemplates(
    saved.annotationPromptTemplates,
    saved.annotationPromptTemplate,
    initialLocale,
  )
  state.annotationPromptTemplates = templateMigration.templates
  if (templateMigration.migratedLegacy && state.language === 'system') {
    state.language = templateMigration.legacyLocale
    initialLocale = resolveLanguage(state.language)
  }
  if (!state.annotationPromptTemplates[initialLocale]) {
    state.annotationPromptTemplates[initialLocale] = defaultAnnotationPrompt(initialLocale)
  }
  state.annotationPromptTemplate = state.annotationPromptTemplates[initialLocale]
  state.openingMessages = migrateLegacyResponsibilities(
    normalizeOpeningMessages(saved.openingMessages),
    saved.router,
  )
  state.router = normalizeThreadRouter(saved.router)
  preferencesReady = true
  applySidebarState()
}

function preferencesSnapshot() {
  return {
    language: state.language,
    theme: state.theme,
    contentWidth: state.contentWidth,
    hiddenSessionDirectories: state.hiddenSessionDirectories,
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
    desktopNotifications: state.desktopNotifications,
    browser: state.browser,
    selectedThread: state.selectedByBackend.codex,
    selectedBackend: state.backend,
    selectedThreads: Object.fromEntries(Object.entries(state.selectedByBackend).filter(([, id]) => typeof id === 'string' && id)),
    annotationDrafts: state.annotationDrafts,
    annotationAdditional: state.annotationAdditional,
    annotationPromptTemplate: state.annotationPromptTemplate,
    annotationPromptTemplates: state.annotationPromptTemplates,
    openingMessages: state.openingMessages,
    router: Object.keys(state.router.controllers).length || state.router.fallbacks.length ? state.router : null,
  }
}

function persistPreferences() {
  if (!preferencesReady) return Promise.resolve()
  const body = JSON.stringify(preferencesSnapshot())
  const write = preferencesWriteChain.catch(() => {}).then(async () => {
    const response = await gatewayFetch('/studio/preferences', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
  })
  preferencesWriteChain = write.catch((error) => console.error('Unable to persist preferences', error))
  return write
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
  $('#annotation-template').value = state.annotationPromptTemplate
  $('#settings-error').classList.add('hidden')
  activateSettingsPane(activeSettingsPane)
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
  state.annotationPromptTemplates[previousLocale] = template.slice(0, 32000)
  state.language = normalizeLanguage($('#language-select').value)
  setLanguage(state.language)
  syncEmbeddedBrowserTranslations()
  state.theme = $('#theme-select').value === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth($('#content-width').value)
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
  state.annotationPromptTemplate = nextLocale === previousLocale
    ? template.slice(0, 32000)
    : state.annotationPromptTemplates[nextLocale] || defaultAnnotationPrompt(nextLocale)
  state.annotationPromptTemplates[nextLocale] = state.annotationPromptTemplate
  applyAppearance()
  persistPreferences()
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
    await applyEnvironmentToCodex(root)
    renderProjectEnvironmentEntry()
  }
}

async function activateSelectedEnvironment() {
  const root = selectedThread()?.cwd || ''
  if (!root) {
    state.environmentProfile = null
    renderProjectEnvironmentEntry()
    return
  }
  state.environmentProfile = await fetchEnvironmentProfile(root)
  renderProjectEnvironmentEntry()
  if (state.environmentProfile?.configured) await applyEnvironmentToCodex(state.environmentProfile.root)
}

function renderProjectEnvironmentEntry() {
  const action = $('#project-environment-action')
  const thread = selectedThread()
  const root = thread?.cwd || ''
  if (!action) return
  action.disabled = !root
  action.title = t(root ? 'Configure variables, secrets, network, and cache for this project' : 'The current session has no project directory')
  const configured = Boolean(root && state.environmentProfile?.configured && state.environmentProfile.root === root)
  $('#project-environment-indicator').classList.toggle('hidden', !configured)
}

async function applyEnvironmentToCodex(root) {
  if (!isCodexBackend(state.backend) || !state.selectedId) return
  const applied = await gatewayFetch('/studio/environment/apply', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, threadId: state.selectedId, backend: state.backend }),
  })
  const result = await applied.json().catch(() => null)
  if (!applied.ok) throw new Error(result?.error?.message || `HTTP ${applied.status}`)
}

function resetSettings() {
  state.language = 'system'
  setLanguage(state.language)
  syncEmbeddedBrowserTranslations()
  state.theme = 'light'
  state.contentWidth = 'comfortable'
  state.typography = { ...typographyDefaults }
  state.desktopNotifications = false
  state.wsl = { distribution: '', user: '', codexBinary: 'codex', opencodeBinary: 'opencode' }
  state.annotationPromptTemplates = { ...annotationPromptDefaults }
  state.annotationPromptTemplate = defaultAnnotationPrompt()
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
  root.style.setProperty('--content-font-family', state.typography.contentFontFamily)
  root.style.setProperty('--content-font-size', `${state.typography.contentFontSize}px`)
  root.style.setProperty('--content-font-weight', state.typography.contentFontWeight)
  root.style.setProperty('--activity-font-weight', Math.max(400, state.typography.contentFontWeight - 100))
  root.style.setProperty('--code-font-family', state.typography.codeFontFamily)
  root.style.setProperty('--code-font-size', `${state.typography.codeFontSize}px`)
  root.style.setProperty('--code-font-weight', state.typography.codeFontWeight)
  workspaceTools.refreshTypography()
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
    <div class="detail-row"><span>${t('Runtime mode')}</span><strong>${t('Local workspace')}</strong></div>
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

function updateSelectedThreadStatus(message) {
  const thread = selectedThread()
  if (!thread) return
  const status = selectedThreadStatusChange(message, thread.id)
  if (status == null) return
  thread.status = status
  renderWorkspace()
}

function threadTitle(thread) { return thread?.name || thread?.preview || basename(thread?.cwd) || thread?.id || t('Codex session') }
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
