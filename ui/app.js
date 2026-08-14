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
  normalizeOpenCodeSessions,
  openCodeModelList,
  openCodeThreadFromHistory,
  splitOpenCodeModel,
} from './opencode-native.mjs'
import {
  composerTrigger,
  fuzzyFileLabel,
  previewableFileKind,
  matchingSkills,
  matchingSlashCommands,
  replaceComposerTrigger,
  selectedFileReference,
  selectedSkillReference,
  shellCommandFromComposer,
  transcriptUpdateKind,
} from './composer-tools.mjs'
import {
  artifactSearchAvailable,
  createFileRangeTarget,
  fileDisplayName,
  findTextMatchRanges,
  isHtmlFile,
  isMarkdownFile,
  STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  STATIC_HTML_FORBIDDEN_TAGS,
} from './document-review.mjs'
import {
  CommentSourceRegistry,
  commentSelectionSnapshot,
  createCommentDraft,
  normalizeCommentDrafts,
} from './comment-core.mjs'
import {
  chatCommentSource,
  createChatCommentProvider,
  createDocumentCommentProvider,
  documentCommentSource,
  legacyCommentSource,
  relocateDocumentComment,
} from './comment-source-providers.mjs'
import { browserCommentSource, createBrowserCommentProvider } from './browser-comment-provider.mjs'
import { createEpubCommentProvider, epubCommentSource } from './epub-comment-provider.mjs'
import { createPdfCommentProvider, pdfCommentSource } from './pdf-comment-provider.mjs'
import { createTableCommentProvider, tableCommentSource } from './table-comment-provider.mjs'
import {
  autoFavoriteTitle,
  favoriteCopyText,
  favoriteSourceKey,
  normalizeFavoriteTags,
  questionForTurn,
} from './favorites.mjs'
import {
  MERMAID_PREFERENCES_DEFAULTS,
  mermaidInitializeConfig,
  normalizeMermaidPreferences,
} from './mermaid-config.mjs'
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
  reasoningStage,
  shouldShowTurnPlaceholder,
} from './transcript-presentation.mjs'
import {
  isTurnForkable,
  openCodeForkBody,
  threadForkParams,
} from './thread-fork.mjs'

import {
  formatDate as formatLocalizedDate,
  getLocale,
  migrateLocalizedTemplates,
  resolveLanguage,
  setLanguage,
  startTranslationObserver,
  t,
} from './i18n.mjs'
import { createTranscriptScrollFollower } from './transcript-scroll.mjs'
import { createWorkspaceTools } from './workspace-tools.mjs'
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
  DEFAULT_FALLBACK_CONDITION,
  finalAgentText,
  isRouterSession,
  managedRouterThread,
  migrateLegacyResponsibilities,
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

const sessionDispatch = new SessionDispatchRegistry()
  .register('codex', {
    read: (ref) => dispatchBackendRpc(ref.backend, 'thread/read', { threadId: ref.id, includeTurns: true }),
    prepareTurn: (ref) => dispatchBackendRpc(ref.backend, 'thread/resume', { threadId: ref.id }),
    startTurn: (ref, input, options = {}) => dispatchBackendRpc(ref.backend, 'turn/start', {
      threadId: ref.id,
      clientUserMessageId: options.clientUserMessageId || randomId(),
      input,
      ...(options.additionalContext ? { additionalContext: options.additionalContext } : {}),
      ...(options.outputSchema ? { outputSchema: options.outputSchema } : {}),
      ...(options.turnOptions || {}),
    }, options.timeoutMs),
  })
  .register('opencode', {
    read: (ref) => dispatchBackendRpc(ref.backend, 'thread/read', { threadId: ref.id, includeTurns: true }),
    startTurn: async (ref, input, options = {}) => {
      const clientUserMessageId = options.clientUserMessageId || randomId()
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

const typographyDefaults = Object.freeze({
  uiFontFamily: 'Ubuntu, "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif',
  uiFontWeight: 500,
  workspaceFontFamily: 'Ubuntu, "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif',
  workspaceFontSize: 14,
  codeFontFamily: '"JetBrains Mono", "SFMono-Regular", Consolas, monospace',
  codeFontSize: 14,
  codeFontWeight: 500,
  highContrast: true,
})

const annotationPromptDefaults = Object.freeze({
  'zh-CN': `请根据下面引用的会话输出或项目文件内容，以及我的批注进行回应。请逐项处理，不要遗漏。涉及文件时，请先读取当前版本并依据文件路径、引用和上下文定位内容；如果文件已变化，以当前内容为准谨慎修改。

{{annotations}}

{{additional}}`,
  'en-US': `Please respond to the quoted conversation output or project file content and my comments below. Address every item. For file comments, read the current version first and locate the passage using its path, quote, and context; if the file changed, modify the current content carefully.

{{annotations}}

{{additional}}`,
})

function defaultAnnotationPrompt(locale = getLocale()) {
  return annotationPromptDefaults[locale] || annotationPromptDefaults['en-US']
}

const state = {
  backend: 'codex',
  selectedByBackend: { codex: null, opencode: null },
  socket: null,
  eventSource: null,
  socketGeneration: 0,
  reconnectTimer: null,
  ready: false,
  backendInfo: null,
  backendInfos: { codex: null, opencode: null },
  hostPlatform: window.__CODEX_THREAD_STUDIO_GATEWAY__?.hostPlatform || null,
  wsl: { distribution: '', user: '', codexBinary: 'codex', opencodeBinary: 'opencode' },
  backendStates: {
    codex: { kind: 'checking', label: '正在启动 Codex', caption: 'App Server · stdio' },
    opencode: { kind: 'idle', label: 'OpenCode', caption: '按需连接' },
  },
  requestId: 0,
  pending: new Map(),
  threads: [],
  threadsByBackend: { codex: [], opencode: [] },
  threadModels: new Map(),
  threadLoads: new Map(),
  selectedId: null,
  search: '',
  filter: 'all',
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
  lastAppServerGeneration: null,
  environmentProfile: null,
  browser: null,
  browserInfo: null,
  embeddedBrowserVisible: false,
  embeddedBrowserLoaded: false,
  activeRightWorkspace: null,
  annotationDrafts: {},
  annotationAdditional: {},
  annotationPromptTemplates: {},
  annotationPromptTemplate: annotationPromptDefaults['zh-CN'],
  openingMessages: {},
  pendingSelection: null,
  pendingAnnotation: null,
  artifact: null,
  artifactView: 'preview',
  artifactSearch: '',
  artifactSearchMatches: [],
  artifactSearchIndex: -1,
  composerMenu: { type: null, trigger: null, options: [], selected: 0, generation: 0 },
  skillCatalog: { cwd: null, skills: [], request: null, loaded: false },
  turnOptions: {},
  pendingSkills: {},
  pendingFiles: {},
  favorites: [],
  favoriteIndex: [],
  favoriteTotal: 0,
  favoriteQuery: '',
  favoriteScope: 'global',
  pendingFavorite: null,
  selectedFavorite: null,
  favoriteEditMode: false,
  sessionMaps: new Map(),
  sessionMapLoads: new Map(),
  sessionMapDismissed: new Set(),
  sessionMapSelectedItem: null,
  sessionMapMenuItem: null,
  sessionMapSync: new Map(),
  sessionMapWorkerRequests: new Map(),
  sessionMapWorkers: new SessionMapWorkerPool(),
  hiddenCodexThreads: new Set(),
  hiddenCodexTurns: new Set(),
  sessionMapBootstrapAttempts: new Set(),
  sessionMapInlineProcessing: new Set(),
  router: normalizeThreadRouter(null),
  routerPending: new Map(),
  routerDispatches: new Map(),
  routerTargetTurns: new Map(),
  routerMonitors: new Map(),
  routerEditor: null,
}

let preferencesReady = false
let preferencesWriteChain = Promise.resolve()
let annotationPersistTimer = null
let transcriptFrame = null
const dirtyStreamItems = new Map()
const turnLatencyTraces = new Map()
let composerSearchTimer = null
let artifactSearchTimer = null
let turnNavigatorFrame = null
let openCodeListRefreshTimer = null
const openCodeStatusReconcileTimers = new Map()
let threadCatalogRetryTimer = null
let threadCatalogRetryAttempt = 0
let threadCatalogErrorMessage = null
const catalogRefreshes = new Map()
const catalogRequestGenerations = new Map()
let favoritesSearchTimer = null
let sessionMapRequestId = -8_500_000
const transcriptScrollFollower = createTranscriptScrollFollower()
const transcriptPresentationCache = new TranscriptPresentationCache({ visibleTurns: 30 })
const markdownRenderCache = new Map()
const MAX_MERMAID_SOURCE_CHARS = 100_000
let mermaidObserver = null
let mermaidRenderChain = Promise.resolve()
let mermaidRenderSequence = 0
let mermaidGeneration = 0
let mermaidInitializedConfig = ''
let activityLogContext = null
let rightRailResize = null
let artifactEditor = null
let epubReader = null
let epubReaderModule = null
let epubReaderGeneration = 0
let epubReadingStateTimer = null
let richArtifactReader = null
let pdfReaderModule = null
let tableReaderModule = null
let workspaceEditorModule = null
let embeddedBrowserWidthTimer = null
let environmentDialogRoot = ''
let environmentDialogProfile = null
const environmentSecretRemovals = new Set()

const workspaceTools = createWorkspaceTools({
  gatewayFetch,
  gatewayWebSocket,
  getThread: selectedThread,
  getBackend: () => state.backend,
  openFile: (file, context = {}) => openArtifact(file, { returnTool: context.returnTool }),
  canOpenFile: (file) => Boolean(previewableFileKind(file)),
  closePeerRails: closeWorkspacePeerRails,
  translate: t,
  notify: toast,
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
  await loadPreferences()
  setLanguage(state.language)
  startTranslationObserver()
  applyAppearance()
  startMermaidRendering()
  applyBackendCopy()
  await loadBrowserInfo().catch((error) => console.warn('Unable to load Browser info', error))
  await loadFavorites().catch(showError)
  // Catalog discovery is independent of the active backend connection. A
  // transient failure in one backend must not leave the whole sidebar empty
  // while the other backend is healthy.
  refreshInactiveCatalog()
  // Codex metadata is not required before its WebSocket connects. OpenCode
  // loads its metadata as part of connectOpenCode(), so it is not requested
  // twice and cannot block initial catalog discovery.
  if (state.backend === 'codex') loadBackendInfo()
  connectBackend()
}

function bindUI() {
  workspaceTools.bind()
  $('#new-thread').addEventListener('click', openNewThreadDialog)
  $('#studio-menu-button').addEventListener('click', () => {
    toggleActionMenu('studio-menu', 'studio-menu-button')
  })
  $('#toggle-sidebar').addEventListener('click', toggleSidebar)
  $('#empty-new-thread').addEventListener('click', openNewThreadDialog)
  $('#close-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#cancel-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#new-thread-form').addEventListener('submit', createThread)
  $('#new-thread-backend').addEventListener('change', updateNewThreadCapabilities)
  $('#thread-search').addEventListener('input', (event) => {
    state.search = event.target.value.trim().toLowerCase()
    renderThreadList()
  })
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
  $('#router-settings-action').addEventListener('click', openRouterDialog)
  $('#router-form').addEventListener('submit', saveRouterSettings)
  $('#close-router-dialog').addEventListener('click', closeRouterDialog)
  $('#cancel-router').addEventListener('click', closeRouterDialog)
  $('#router-add-fallback').addEventListener('click', addRouterFallback)
  $('#session-map-action').addEventListener('click', handleSessionMapAction)
  $('#session-map-more').addEventListener('click', () => toggleActionMenu('session-map-menu', 'session-map-more'))
  $('#close-session-map').addEventListener('click', closeSessionMapRail)
  $('#session-map-add-root').addEventListener('click', () => openSessionMapItemDialog())
  $('#session-map-ai-generate').addEventListener('click', () => generateSessionMapStructure().catch(showError))
  $('#session-map-edit-goal').addEventListener('click', openSessionMapGoalDialog)
  $('#session-map-undo').addEventListener('click', () => undoSessionMap().catch(showError))
  $('#session-map-delete').addEventListener('click', () => deleteSessionMap().catch(showError))
  $('#session-map-tree').addEventListener('click', handleSessionMapTreeClick)
  $('#session-map-item-menu').addEventListener('click', handleSessionMapItemMenu)
  $('#session-map-form').addEventListener('submit', createSessionMap)
  $('#close-session-map-dialog').addEventListener('click', closeSessionMapDialog)
  $('#cancel-session-map').addEventListener('click', closeSessionMapDialog)
  $('#session-map-goal-form').addEventListener('submit', saveSessionMapGoal)
  $('#session-map-suggest-goal').addEventListener('click', () => suggestSessionMapGoal().catch(showError))
  $('#close-session-map-goal').addEventListener('click', () => $('#session-map-goal-dialog').close())
  $('#cancel-session-map-goal').addEventListener('click', () => $('#session-map-goal-dialog').close())
  $('#session-map-item-form').addEventListener('submit', saveSessionMapItem)
  $('#close-session-map-item').addEventListener('click', () => $('#session-map-item-dialog').close())
  $('#cancel-session-map-item').addEventListener('click', () => $('#session-map-item-dialog').close())
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
  $('#composer-menu').addEventListener('mousedown', (event) => event.preventDefault())
  $('#composer-menu').addEventListener('click', handleComposerMenuClick)
  $('#composer-review-open').addEventListener('click', openAnnotationRail)
  $('#composer-review-insert').addEventListener('click', insertAnnotations)
  $('#interrupt-turn').addEventListener('click', interruptTurn)
  $('#turn-navigator-list').addEventListener('click', handleTurnNavigatorClick)
  $('#open-browser-workspace').addEventListener('click', () => openGlobalBrowser().catch(showError))
  $('#transcript').addEventListener('scroll', handleTranscriptScroll, { passive: true })
  $('#transcript').addEventListener('mouseup', captureTranscriptSelection)
  $('#artifact-content').addEventListener('mouseup', captureArtifactSelection)
  $('#artifact-content').addEventListener('click', handleTranscriptClick)
  $('#close-artifact').addEventListener('click', closeArtifactRail)
  $('#refresh-artifact').addEventListener('click', () => refreshArtifact().catch(showError))
  $('#artifact-preview').addEventListener('click', () => setArtifactView('preview'))
  $('#artifact-source').addEventListener('click', () => setArtifactView('source'))
  $('#artifact-edit').addEventListener('click', () => setArtifactView('edit'))
  $('#artifact-save').addEventListener('click', () => saveArtifact().catch(showError))
  $('#artifact-search-input').addEventListener('input', handleArtifactSearchInput)
  $('#artifact-search-input').addEventListener('keydown', handleArtifactSearchKeydown)
  $('#artifact-search-prev').addEventListener('click', () => navigateArtifactSearch(-1))
  $('#artifact-search-next').addEventListener('click', () => navigateArtifactSearch(1))
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
  $('#open-thread-comments').addEventListener('click', openAnnotationRail)
  $('#open-thread-favorites').addEventListener('click', () => openFavoritesRail('session'))
  window.addEventListener('resize', () => {
    scheduleTurnNavigatorSync()
    applyRightRailWidth()
    workspaceTools.resize()
  })
  $('#selection-popover').addEventListener('mousedown', (event) => event.preventDefault())
  $('#selection-comment').addEventListener('click', openAnnotationFromSelection)
  $('#selection-favorite').addEventListener('click', openFavoriteFromSelection)
  $('#close-annotation-rail').addEventListener('click', closeAnnotationRail)
  $('#annotation-form').addEventListener('submit', addAnnotation)
  $('#close-annotation-dialog').addEventListener('click', closeAnnotationDialog)
  $('#cancel-annotation').addEventListener('click', closeAnnotationDialog)
  $('#clear-annotations').addEventListener('click', clearAnnotations)
  $('#insert-annotations').addEventListener('click', insertAnnotations)
  $('#annotation-additional').addEventListener('input', saveAnnotationAdditional)
  $('#open-favorites').addEventListener('click', () => {
    closeActionMenus()
    openFavoritesRail('global')
  })
  $('#export-favorites').addEventListener('click', () => exportFavorites().catch(showError))
  $('#close-favorites').addEventListener('click', closeFavoritesRail)
  $('#favorites-search').addEventListener('input', handleFavoritesSearch)
  $('#favorites-list').addEventListener('click', handleFavoriteListClick)
  $('#favorite-form').addEventListener('submit', saveFavorite)
  $('#close-favorite-dialog').addEventListener('click', closeFavoriteDialog)
  $('#cancel-favorite').addEventListener('click', closeFavoriteDialog)
  $('#favorite-include-question').addEventListener('change', renderFavoriteQuestionOption)
  $('#close-favorite-detail').addEventListener('click', closeFavoriteDetail)
  $('#copy-favorite').addEventListener('click', () => copySelectedFavorite().catch(showError))
  $('#edit-favorite').addEventListener('click', editSelectedFavorite)
  $('#delete-favorite').addEventListener('click', () => deleteSelectedFavorite().catch(showError))
  $('#open-favorite-source').addEventListener('click', () => openSelectedFavoriteSource().catch(showError))
  $('#connections-button').addEventListener('click', openConnectionsDialog)
  $('#close-connections').addEventListener('click', () => $('#connections-dialog').close())
  $('#settings-button').addEventListener('click', () => {
    closeActionMenus()
    openSettings()
  })
  $('#about-button').addEventListener('click', openBackendDialog)
  $('#close-settings').addEventListener('click', () => $('#settings-dialog').close())
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
    if (!event.target.closest('#selection-popover, .content-menu-anchor')) hideSelectionPopover()
    if (!event.target.closest('.menu-anchor')) closeActionMenus()
    if (!event.target.closest('#session-map-item-menu, .session-map-row-menu')) closeSessionMapItemMenu()
  })
  document.addEventListener('keydown', (event) => {
    const modifier = event.ctrlKey || event.metaKey
    if (modifier && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      openNewThreadDialog()
    } else if (modifier && event.key.toLowerCase() === 'b') {
      event.preventDefault()
      toggleSidebar()
    } else if (event.key === '/' && !isTypingTarget(event.target)) {
      event.preventDefault()
      $('#thread-search').focus()
    } else if (event.key === 'Escape') {
      const artifactWasOpen = !$('#artifact-rail').classList.contains('hidden')
      hideSelectionPopover()
      closeActionMenus()
      closeAnnotationRail()
      closeFavoritesRail()
      closeSessionMapItemMenu()
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
  button.title = t(state.sidebarCollapsed ? '展开会话栏 (Ctrl+B)' : '收起会话栏 (Ctrl+B)')
  button.setAttribute('aria-label', t(state.sidebarCollapsed ? '展开会话栏' : '收起会话栏'))
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
  const label = t(!available ? '不可用' : state.embeddedBrowserVisible ? '已显示' : state.embeddedBrowserLoaded ? '已隐藏' : '未启动')
  const button = $('#open-browser-workspace')
  button.disabled = !available
  button.title = `${t('浏览器')} · ${label} · embedded`
  button.setAttribute('aria-label', button.title)
}

async function openGlobalBrowser() {
  closeActionMenus()
  if (!usesEmbeddedBrowser()) throw new Error(t('当前平台不支持嵌入浏览器'))
  const width = currentRightRailPixelWidth()
  activateRightWorkspace('browser')
  dispatchEmbeddedBrowserAction(`studio-action://show-browser?width=${width}`)
}

async function openBrowserUrl(url) {
  if (!usesEmbeddedBrowser()) throw new Error(t('当前平台不支持嵌入浏览器'))
  const width = currentRightRailPixelWidth()
  activateRightWorkspace('browser')
  dispatchEmbeddedBrowserAction(`studio-action://open-browser?url=${encodeURIComponent(String(url || ''))}&width=${width}`)
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

function usesEmbeddedBrowser() {
  return state.browserInfo?.presentation === 'embedded-webview'
}

function openEmbeddedBrowserComment(selection) {
  const excerpt = String(selection?.text || '').trim().slice(0, 16000)
  if (!excerpt) return toast(t('请先在网页中选择文本'), 'error')
  if (!state.selectedId) return toast(t('请先选择一个会话'), 'error')
  state.pendingSelection = {
    quote: excerpt,
    itemId: null,
    turnId: null,
    source: browserCommentSource({
      url: selection.url,
      title: selection.title,
    }),
  }
  openAnnotationFromSelection()
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

function backendDescriptor(backend) {
  return backend === 'opencode'
    ? { id: 'opencode', name: 'OpenCode', nativeLabel: 'OPENCODE NATIVE', binary: 'opencode', infoPath: '/studio/opencode', protocol: 'OpenCode Server API', transport: 'HTTP + SSE' }
    : { id: 'codex', name: 'Codex', nativeLabel: 'CODEX NATIVE', binary: 'codex', infoPath: '/studio/codex', protocol: 'Codex App Server v2', transport: 'stdio JSONL' }
}

function currentBackend() {
  return backendDescriptor(state.backend)
}

function connectBackend() {
  if (state.backend === 'opencode') connectOpenCode().catch(showError)
  else connectAppServer()
}

async function switchBackend(backend, { selectedId } = {}) {
  if (!['codex', 'opencode'].includes(backend) || backend === state.backend) return
  const previousBackend = state.backend
  state.selectedByBackend[state.backend] = state.selectedId
  cleanupConnections()
  state.backendStates[previousBackend] = { kind: 'idle', label: backendDescriptor(previousBackend).name, caption: '按需连接' }
  rejectPending(new Error('后端已切换'))
  state.backend = backend
  if (selectedId) state.selectedByBackend[backend] = selectedId
  state.selectedId = state.selectedByBackend[backend] || null
  state.threads = state.threadsByBackend[backend]
  state.model = freshThreadModel(backend, state.selectedId)?.model || createCodexViewModel()
  if (state.selectedId) state.model.threadId = state.selectedId
  state.backendInfo = state.backendInfos[backend]
  state.ready = false
  applyBackendCopy()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  renderSessionMap()
  persistPreferences()
  await loadBackendInfo()
  connectBackend()
}

function applyBackendCopy() {
  const descriptor = currentBackend()
  $('#empty-mark').textContent = descriptor.id === 'codex' ? 'C' : 'O'
  $('#tool-avatar').textContent = descriptor.id === 'codex' ? 'CX' : 'OC'
  $('#new-thread-label').textContent = t('新建会话')
  $('#native-error-title').textContent = t('{backend} Server 无法使用', { backend: descriptor.name })
  $('#empty-title').textContent = t('结构化 {backend} 工作台', { backend: descriptor.name })
  $('#empty-description').textContent = descriptor.id === 'codex'
    ? '消息、命令、文件修改、计划、审批和停止原因直接来自 Codex App Server。'
    : '消息、工具、文件修改、权限和停止原因直接来自 OpenCode Server，保留结构化事件。'
  $('#composer-input').placeholder = t('向 {backend} 发送消息… @ 文件 · $ 技能 · / 命令 · ! Shell', { backend: descriptor.name })
  $('#rename-thread-description').textContent = t('名称由 {backend} 持久化。', { backend: descriptor.name })
  const wsl = state.backendInfo?.executionEnvironment === 'wsl'
  $('#new-thread-cwd').placeholder = '/home/user/projects/project'
  $('#new-thread-cwd-help').textContent = t(wsl
    ? '填写 WSL 中的 Linux 绝对路径，例如 /home/user/project。'
    : '必须是本机绝对路径。')
}

function connectAppServer() {
  clearTimeout(state.reconnectTimer)
  cleanupSocket()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  setBackendState('checking', '正在启动 Codex', 'App Server · stdio')
  setNativeError(null)
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/codex`)
  state.socket = socket

  socket.onmessage = (event) => {
    if (generation !== state.socketGeneration) return
    try { handleAppServerMessage(JSON.parse(event.data)) }
    catch (error) { console.error('Invalid App Server message', error, event.data) }
  }
  socket.onerror = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('error', 'Codex 未连接', 'WebSocket 连接失败')
  }
  socket.onclose = () => {
    if (generation !== state.socketGeneration) return
    state.ready = false
    rejectPending(new Error('Codex App Server connection closed'))
    setBackendState('error', 'Codex 已断开', '正在准备重连…')
    setNativeError('与本机 Codex App Server 的连接已断开。')
    state.reconnectTimer = setTimeout(connectAppServer, 1800)
  }
}

async function connectOpenCode() {
  clearTimeout(state.reconnectTimer)
  cleanupConnections()
  state.ready = false
  state.socketGeneration += 1
  const generation = state.socketGeneration
  setBackendState('checking', '正在启动 OpenCode', 'Server · HTTP/SSE')
  setNativeError(null)
  await loadBackendInfo()
  if (generation !== state.socketGeneration) return
  if (state.backendInfo?.reachable === false || state.backendInfo?.error) {
    const reason = state.backendInfo.error || 'OpenCode Server 未就绪'
    setBackendState('error', 'OpenCode 不可用', reason)
    setNativeError(reason)
    return
  }
  state.ready = true
  setBackendState('online', 'OpenCode Server', '原生结构化连接')
  $('#native-connection').textContent = '已连接'
  const events = gatewayEventSource('/opencode/global/event')
  state.eventSource = events
  events.onopen = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('online', 'OpenCode Server', '原生结构化连接')
    $('#native-connection').textContent = '已连接'
  }
  events.onmessage = (event) => {
    if (generation !== state.socketGeneration) return
    try { handleOpenCodeServerEvent(JSON.parse(event.data)) }
    catch (error) { console.error('Invalid OpenCode SSE event', error, event.data) }
  }
  events.onerror = () => {
    if (generation !== state.socketGeneration) return
    setBackendState('checking', 'OpenCode 正在重连', 'SSE 事件流')
    $('#native-connection').textContent = '正在重连事件流…'
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
  if (message.method === 'studio/appServer/status') {
    const status = message.params?.state
    if (status === 'ready') {
      const firstReady = !state.ready
      const reconnecting = state.lastAppServerGeneration != null
      const nextGeneration = message.params?.generation ?? state.lastAppServerGeneration
      if (state.lastAppServerGeneration != null && nextGeneration !== state.lastAppServerGeneration) {
        sessionDispatch.clearPrepared('codex')
      }
      state.lastAppServerGeneration = nextGeneration
      state.appServerCapabilities = { ...(message.params?.clientCapabilities || {}) }
      state.appServerInitialization = message.params?.initialization || null
      state.ready = true
      setBackendState('online', 'Codex App Server', '原生结构化连接')
      $('#native-connection').textContent = '已连接'
      setNativeError(null)
      if (firstReady) {
        loadThreads().then(async () => {
          if (reconnecting && state.selectedId) await refreshSelectedThread({ quiet: true })
        }).catch((error) => handleThreadCatalogFailure('codex', state.socketGeneration, error))
      }
    } else if (status === 'starting') {
      setBackendState('checking', '正在启动 Codex', message.params?.binary || 'App Server')
    } else if (status === 'error' || status === 'stopped') {
      sessionDispatch.clearPrepared('codex')
      const reason = message.params?.message || message.params?.reason || 'App Server 已停止'
      setBackendState('error', 'Codex 不可用', reason)
      setNativeError(reason)
    }
    return
  }
  if (message.method === 'studio/appServer/log') {
    console.debug('codex app-server', message.params?.line)
    return
  }
  if (message.method === 'studio/appServer/lagged') {
    const skipped = Number(message.params?.skipped || 0)
    if (!state.selectedId) {
      toast(t('界面错过了 {count} 条 App Server 事件', { count: skipped }), 'error')
      return
    }
    setNativeError(t('界面错过了 {count} 条 App Server 事件，正在从 Codex 重新同步当前会话…', { count: skipped }))
    refreshSelectedThread({ quiet: true }).then((refreshed) => {
      if (!refreshed) return
      setNativeError(null)
      toast('已从 Codex 重新同步会话')
    })
    return
  }
  if (message.method?.startsWith('studio/appServer/')) {
    const error = message.params?.message || message.method
    setNativeError(error)
    return
  }

  if (message.id != null && !message.method) {
    captureSessionMapWorkerResponse(message)
    const pending = state.pending.get(String(message.id))
    if (!pending) return
    state.pending.delete(String(message.id))
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(message.error.message || JSON.stringify(message.error)))
    else pending.resolve(message.result)
    return
  }

  if (message.method === 'thread/started' && message.params?.thread) {
    if (message.params.thread.ephemeral || state.hiddenCodexThreads.has(String(message.params.thread.id))) return
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
  if (message.method === 'thread/archived' || message.method === 'thread/deleted') {
    const threadId = message.params?.threadId
    state.threads = state.threads.filter((thread) => thread.id !== threadId)
    state.threadsByBackend.codex = state.threads
    if (message.method === 'thread/deleted') {
      delete state.annotationDrafts[`codex:${threadId}`]
      delete state.annotationAdditional[`codex:${threadId}`]
      delete state.openingMessages[`codex:${threadId}`]
    }
    const deletedKey = sessionRefKey('codex', threadId)
    if (state.router.controllers.codex === threadId) {
      const controllers = { ...state.router.controllers }
      delete controllers.codex
      state.router = normalizeThreadRouter({ ...state.router, controllers })
    } else if (state.router.fallbacks.some((entry) => entry.sessionKey === deletedKey)) {
      state.router = normalizeThreadRouter({ ...state.router, fallbacks: state.router.fallbacks.filter((entry) => entry.sessionKey !== deletedKey) })
    }
    invalidateThreadModel('codex', threadId)
    persistPreferences()
    if (state.selectedId === threadId) {
      state.selectedId = null
      state.selectedByBackend.codex = null
      state.model = createCodexViewModel()
      persistPreferences()
    }
    renderThreadList()
    renderWorkspace()
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
      handleSessionMapToolCall(message)
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
      toast(t('Codex 请求了尚未支持的交互：{method}', { method: message.method }), 'error')
      return
    }
    if (message.method === 'item/tool/requestUserInput' || message.method === 'mcpServer/elicitation/request') {
      notifyDesktop(t('Codex 正在等待你的输入'), message.params?.questions?.[0]?.question || message.params?.message || selectedThread()?.name || '')
    }
    markCachedModelValidated('codex', targetModel)
    if (targetModel !== state.model) return
    renderTranscript()
    return
  }

  const targetModel = codexNotificationModel(message)
  if (!targetModel) return
  if (applyCodexNotification(targetModel, message)) {
    markCachedModelValidated('codex', targetModel)
    if (message.method === 'turn/completed') {
      const completedThread = state.threads.find((thread) => thread.id === (message.params?.threadId || targetModel.threadId))
      notifyDesktop(t('任务已完成'), threadTitle(completedThread || { name: t('未命名会话') }))
      completeRouterTurn({
        backend: 'codex',
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
      processSessionMapInlineUpdate('codex', threadId, targetModel, message.params?.turn?.id).catch((error) => {
        console.warn('Session Map inline update failed', error)
      })
    }
  }
}

async function captureOffscreenInteraction(message) {
  const threadId = String(message.params?.threadId || '')
  if (!threadId) return
  const result = await rpc('thread/read', { threadId, includeTurns: true })
  const model = createCodexViewModel()
  hydrateCodexThread(model, result.thread)
  applyCodexNotification(model, message)
  cacheThreadModel('codex', threadId, model)
  state.attentionThreads.add(threadCatalogKey('codex', threadId))
  persistPreferences()
  renderThreadList()
  notifyDesktop(t('Codex 正在等待你的输入'), message.params?.questions?.[0]?.question || message.params?.message || threadTitle(result.thread))
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

function captureSessionMapWorkerResponse(message) {
  const id = String(message?.id ?? '')
  const method = state.sessionMapWorkerRequests.get(id)
  if (!method) return
  state.sessionMapWorkerRequests.delete(id)
  if (message.error) return
  if (method === 'thread/start' && message.result?.thread?.id) {
    state.hiddenCodexThreads.add(String(message.result.thread.id))
  }
  if (method === 'turn/start' && message.result?.turn?.id) {
    state.hiddenCodexTurns.add(String(message.result.turn.id))
  }
}

async function handleSessionMapToolCall(message) {
  const params = message.params || {}
  const key = sessionMapKey('codex', params.threadId)
  try {
    const map = await loadSessionMap('codex', params.threadId)
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
    state.sessionMapSync.set(key, { state: 'synced', message: 'Map 已在当前 Turn 中更新' })
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

function handleOpenCodeServerEvent(event) {
  const payload = event?.payload || event
  if (!payload?.type || payload.type === 'sync' || payload.type === 'server.heartbeat') return
  if (payload.type === 'server.connected') {
    scheduleOpenCodeListRefresh()
    return
  }
  if (payload.type.startsWith('session.')) scheduleOpenCodeListRefresh()
  if (payload.type === 'session.deleted') {
    const deletedId = payload.properties?.info?.id || payload.properties?.sessionID
    const deletedKey = sessionRefKey('opencode', deletedId)
    delete state.openingMessages[deletedKey]
    if (state.router.controllers.opencode === deletedId) {
      const controllers = { ...state.router.controllers }
      delete controllers.opencode
      state.router = normalizeThreadRouter({ ...state.router, controllers })
      persistPreferences()
    } else if (state.router.fallbacks.some((entry) => entry.sessionKey === deletedKey)) {
      state.router = normalizeThreadRouter({ ...state.router, fallbacks: state.router.fallbacks.filter((entry) => entry.sessionKey !== deletedKey) })
      persistPreferences()
    }
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
  const eventThreadId = openCodeEventThreadId(payload)
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
  if (!state.ready || state.socket?.readyState !== WebSocket.OPEN) return Promise.reject(new Error('Codex App Server 尚未就绪'))
  const id = ++state.requestId
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      state.pending.delete(String(id))
      reject(new Error(t('{method} 请求超时', { method })))
    }, timeoutMs)
    state.pending.set(String(id), { resolve, reject, timer, method })
    sendRaw({ id, method, params })
  })
}

async function dispatchBackendRpc(backend, method, params = {}, timeoutMs = 30_000) {
  if (backend === state.backend && state.ready) return rpc(method, params, timeoutMs)
  if (backend === 'codex') return codexBackgroundRpc(method, params, timeoutMs)
  if (backend === 'opencode') {
    await ensureOpenCodeAvailable()
    return openCodeRpc(method, params, timeoutMs, { allowInactive: true })
  }
  throw new Error(`Unsupported session backend: ${backend}`)
}

function codexBackgroundRpc(method, params = {}, timeoutMs = 30_000) {
  return new Promise((resolve, reject) => {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/codex`)
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
    const timer = setTimeout(() => finish(new Error(t('{method} 请求超时', { method }))), timeoutMs)
    socket.onmessage = (event) => {
      let message
      try { message = JSON.parse(event.data) } catch { return }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'error') {
        finish(new Error(message.params?.message || 'Codex App Server 不可用'))
      } else if (message.method === 'studio/appServer/status' && message.params?.state === 'ready' && !requested) {
        requested = true
        socket.send(JSON.stringify({ id, method, params }))
      } else if (message.id === id) {
        if (message.error) finish(new Error(message.error.message || JSON.stringify(message.error)))
        else finish(null, message.result)
      }
    }
    socket.onerror = () => finish(new Error('无法连接 Codex App Server'))
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
  if (!allowInactive && (state.backend !== 'opencode' || !state.ready)) throw new Error('OpenCode Server 尚未就绪')
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
    if (error.name === 'AbortError') throw new Error(t('{method} {path} 请求超时', { method, path }))
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
  const sessions = await openCodeFetch(`/experimental/session?limit=${limit}&archived=false`, options)
  if (!includeStatuses) return normalizeOpenCodeSessions(sessions, {})
  const directories = [...new Set((sessions || []).map((session) => session.directory).filter(Boolean))]
  const statusMaps = await Promise.all(directories.map((cwd) =>
    openCodeFetch(withDirectory('/session/status', cwd), options).catch(() => ({})),
  ))
  return normalizeOpenCodeSessions(sessions, Object.assign({}, ...statusMaps))
}

function fetchCodexCatalog(limit = 100, { routerId = null, routerWorkspace = '' } = {}) {
  return new Promise((resolve, reject) => {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/codex`)
    const id = -(Date.now() + Math.floor(Math.random() * 100_000))
    const routerReadId = id - 1
    const timer = setTimeout(() => finish(new Error('Codex 会话目录请求超时')), 15_000)
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
        finish(new Error(message.params?.message || 'Codex App Server 不可用'))
        return
      }
      if (message.method === 'studio/appServer/status' && message.params?.state === 'ready' && !requested) {
        requested = true
        socket.send(JSON.stringify({ id, method: 'thread/list', params: { limit } }))
        return
      }
      if (message.id === id) {
        if (message.error) {
          finish(new Error(message.error.message || 'Codex 会话目录请求失败'))
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
    socket.onerror = () => finish(new Error('无法读取 Codex 会话目录'))
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
  if (backend === 'codex') {
    const info = await loadBackendInfo('codex')
    return fetchCodexCatalog(100, {
      routerId: state.router.controllers.codex || null,
      routerWorkspace: info?.routerWorkspace || '',
    })
  }
  throw new Error(t('不支持的会话后端：{backend}', { backend }))
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

function installBackendCatalog(backend, threads) {
  state.threadsByBackend[backend] = threads
  if (backend === state.backend) state.threads = threads
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
    throw new Error(t('无法刷新完整会话目录：{message}', { message: error.message }))
  }
}

async function refreshInactiveCatalog() {
  const backend = state.backend === 'codex' ? 'opencode' : 'codex'
  try {
    await refreshBackendCatalog(backend)
    if (backend === state.router.controllerBackend) await ensureManagedRouterSession(backend)
  } catch (error) {
    console.warn(`Unable to refresh ${backend} catalog`, error)
    reportClientError(new Error(`${backend} inactive session catalog failed: ${error?.message || error}`))
  }
}

function handleThreadCatalogFailure(backend, generation, error) {
  if (backend !== state.backend || generation !== state.socketGeneration || !state.ready) return
  threadCatalogRetryAttempt += 1
  threadCatalogErrorMessage = t('无法加载 {backend} 会话列表，Studio 将自动重试。', {
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
  if (method === 'thread/archive') throw new Error('OpenCode 当前没有独立归档操作；可重命名、Fork 或删除会话。')
  if (method === 'turn/start') {
    const text = textFromUserContent(params.input)
    const model = splitOpenCodeModel(params.model)
    const skill = params.input?.find((item) => item?.type === 'skill')
    if (skill?.name) {
      return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/command`, directory), {
        method: 'POST',
        body: { command: skill.name, arguments: text, agent: 'build' },
        timeoutMs: Math.max(timeoutMs, 300_000),
      })
    }
    const parts = [{ type: 'text', text }, ...(params.input || []).filter((item) => item?.type === 'file').map(openCodeFilePart)]
    await openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/prompt_async`, directory), {
      method: 'POST',
      body: {
        parts,
        ...(params.clientUserMessageId ? { messageID: params.clientUserMessageId } : {}),
        ...(model ? { model } : {}),
        ...(params.developerInstructions ? { system: params.developerInstructions } : {}),
        ...(params.outputSchema ? { format: { type: 'json_schema', schema: params.outputSchema, retryCount: 2 } } : {}),
      },
      timeoutMs,
      allowInactive,
    })
    return null
  }
  if (method === 'turn/steer') throw new Error('OpenCode 正在运行时不能追加消息；请等待完成或先停止。')
  if (method === 'turn/interrupt') return openCodeFetch(withDirectory(`/session/${encodeURIComponent(params.threadId)}/abort`, directory), { method: 'POST', timeoutMs })
  if (method === 'thread/shellCommand') {
    const options = currentTurnOptions()
    const model = splitOpenCodeModel(options.model || thread?.model)
    if (!model) throw new Error('运行 OpenCode Shell 前请先通过 /model 选择模型。')
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
  throw new Error(t('OpenCode 后端尚未支持 {method}', { method }))
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
  const result = await rpc('thread/list', { limit: 100 })
  setActiveThreads(Array.isArray(result?.data) ? result.data : [])
  markThreadCatalogLoaded()
  if (state.backend === state.router.controllerBackend) {
    await ensureManagedRouterSession(state.backend).catch(showError)
  }
  renderThreadList()
  refreshInactiveCatalog()
  const visibleThreads = sidebarThreadsForBackend(state.backend).filter((thread) => !isSessionDirectoryHidden(thread.cwd, state.hiddenSessionDirectories, state.sessionDirectoryIgnore))
  const preferred = state.selectedId
  const recent = [...visibleThreads].sort((left, right) => threadUpdatedAt(right) - threadUpdatedAt(left))[0]
  const nextId = visibleThreads.some((thread) => thread.id === preferred) ? preferred : recent?.id
  if (nextId) await selectThread(nextId, { force: true }).catch(showError)
  else renderWorkspace()
}

function setActiveThreads(threads) {
  state.threads = threads
  state.threadsByBackend[state.backend] = threads
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
      ? '没有匹配的会话'
      : state.filter === 'active'
        ? '没有正在运行的会话'
        : state.filter === 'attention'
          ? '没有已加载的会话'
        : '还没有会话'
    list.innerHTML = `<div class="list-empty">${t(message)}</div>`
    return
  }
  const renderRow = ({ backend, thread }) => {
    const status = threadStatus(thread)
    const active = backend === state.backend && thread.id === state.selectedId
    const tag = backend === 'codex' ? 'CX' : 'OC'
    const router = backend === state.router.controllerBackend && isRouterSession(state.router, backend, thread.id)
    return `<button class="thread-row${active ? ' active' : ''}" data-thread-id="${escapeHtml(thread.id)}" data-backend="${backend}">
      <span class="status-dot ${escapeHtml(status)}"></span>
      <span class="thread-copy"><strong>${escapeHtml(threadTitle(thread))}</strong><small data-no-i18n title="${escapeHtml(thread.cwd || t('未记录项目目录'))}">${escapeHtml(thread.cwd || t('未记录项目目录'))}</small></span>
      <span class="thread-tags">${router ? '<span class="backend-tag router" title="Thread Router">RT</span>' : ''}<span class="backend-tag ${backend}" title="${backend === 'codex' ? 'Codex' : 'OpenCode'}">${tag}</span></span>
    </button>`
  }
  if (state.filter === 'attention') {
    list.innerHTML = entries.map(renderRow).join('')
  } else {
    list.innerHTML = groupCatalogEntries(entries).map(({ cwd, name, entries: groupEntries }) => {
      const label = name || t('其他会话')
      const collapsed = state.collapsedThreadGroups.has(cwd)
      return `<section class="thread-group${collapsed ? ' collapsed' : ''}" data-group-path="${escapeHtml(cwd)}">
        <button class="thread-group-heading" type="button" data-no-i18n title="${escapeHtml(cwd || t('未记录项目目录'))}">
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
  closeActionMenus()
  hideComposerMenu()
  resetStreamingPatches()
  transcriptScrollFollower.reset()
  if (state.artifact?.threadKey !== sessionMapKey(state.backend, id)) closeArtifactRail({ restoreMap: false })
  state.selectedId = id
  state.selectedByBackend[state.backend] = id
  setNativeError(null)
  state.sessionMapSelectedItem = null
  closeSessionMapItemMenu()
  const key = sessionMapKey(state.backend, id)
  const mapLoad = loadSessionMap(state.backend, id).catch((error) => {
    console.warn('Unable to load Session Map', error)
    if (state.selectedId === id) setSessionMapSyncState('error', error.message)
  })
  const cached = freshThreadModel(state.backend, id)
  state.model = cached?.model || createCodexViewModel()
  state.model.threadId = id
  persistPreferences()
  renderThreadList()
  renderWorkspace()
  renderTranscript()
  if (cached) {
    $('#native-connection').textContent = t('已从缓存恢复')
    await mapLoad
    await activateSelectedEnvironment().catch((error) => reportClientError(error))
    maybeBootstrapSessionMap(key, state.model)
    return
  }
  await resumeThread(id)
  await mapLoad
  await activateSelectedEnvironment().catch((error) => reportClientError(error))
  maybeBootstrapSessionMap(key, state.model)
}

function markThreadLoaded(backend, id) {
  if (addLoadedThread(state.attentionThreads, backend, id)) renderThreadList()
}

function updateCodexReplyTime(message, model = null) {
  if (message.method !== 'turn/completed') return
  const threadId = message.params?.threadId
    || message.params?.thread?.id
    || message.params?.turn?.threadId
    || threadIdForCachedModel('codex', model)
    || (state.backend === 'codex' ? state.selectedId : null)
  if (!threadId) return
  updateLoadedThreadTimestamp('codex', threadId)
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
  $('#native-connection').textContent = '正在恢复会话…'
  try {
    const result = await rpc('thread/resume', { threadId: id })
    sessionDispatch.markPrepared({ backend: state.backend, id })
    if (state.selectedId !== id) return
    hydrateCodexThread(state.model, result.thread)
    if (state.backend === 'opencode') hydrateOpenCodeModelMetadata(result.thread)
    mergeThreadMetadata(result.thread)
    cacheThreadModel(state.backend, id)
    $('#native-connection').textContent = '已连接'
    renderWorkspace()
    renderTranscript()
  } catch (error) {
    if (state.selectedId !== id) return
    state.model.error = error.message
    state.model.status = 'failed'
    setNativeError(t('无法恢复此 {backend} 会话：{message}', { backend: currentBackend().name, message: error.message }))
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
    if (!quiet) toast('会话已刷新')
    return true
  } catch (error) {
    if (quiet) setNativeError(t('无法重新同步当前 {backend} 会话：{message}', { backend: currentBackend().name, message: error.message }))
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
  const params = message.params || {}
  const explicitId = params.threadId || params.thread?.id || params.turn?.threadId
  if (explicitId && state.hiddenCodexThreads.has(String(explicitId))) return null
  if (explicitId) {
    if (state.backend === 'codex' && state.selectedId === explicitId) return state.model
    return state.threadModels.get(threadCatalogKey('codex', explicitId))?.model || null
  }
  const turnId = params.turnId || params.turn?.id
  if (turnId && state.hiddenCodexTurns.has(String(turnId))) return null
  if (turnId) {
    for (const [key, cached] of state.threadModels) {
      if (!key.startsWith('codex:')) continue
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
  const thread = state.threadsByBackend.codex.find((candidate) => candidate.id === threadId)
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
  if (index >= 0) state.threads[index] = { ...state.threads[index], ...incoming, turns: undefined }
  else state.threads.unshift({ ...incoming, turns: undefined })
  state.threadsByBackend[state.backend] = state.threads
  renderThreadList()
}

function selectedThread() {
  return state.threads.find((thread) => thread.id === state.selectedId) || null
}

function selectedStateKey(id = state.selectedId, backend = state.backend) {
  return id ? `${backend}:${id}` : ''
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
      <header><strong>${t('起始问题')}</strong><span>${opening ? `${opening.source === 'history' ? t('从历史提取') : escapeHtml(opening.source)}${opening.truncated ? ` · ${t('已截断')}` : ''}` : t('尚未识别')}</span></header>
      <textarea id="session-opening-question" rows="4" maxlength="16384" placeholder="${t('当前结构化历史中没有找到用户首条消息。')}">${escapeHtml(opening?.text || '')}</textarea>
    </section>
    ${routable ? `<section class="session-responsibility-card">
      <header><div><strong>${t('会话职责')}</strong><small>${t('供 Thread Router 判断哪些请求应派发到这个会话。')}</small></div><button id="generate-session-responsibility" class="subtle-button compact" type="button">${t('AI 生成')}</button></header>
      <textarea id="session-responsibility" rows="4" placeholder="${t('概括这个会话负责的领域和适合处理的请求')}">${escapeHtml(opening?.responsibility || '')}</textarea>
    </section>` : ''}
    <div class="detail-row"><span>状态</span><strong>${escapeHtml(statusLabel(status))}</strong></div>
    <div class="detail-row"><span>后端</span><strong>${escapeHtml(currentBackend().name)}</strong></div>
    <div class="detail-row"><span>会话 ID</span><strong data-no-i18n>${escapeHtml(thread.id)}</strong></div>
    ${thread.sessionId && thread.sessionId !== thread.id ? `<div class="detail-row"><span>会话树</span><strong data-no-i18n>${escapeHtml(thread.sessionId)}</strong></div>` : ''}
    <div class="detail-row"><span>项目目录</span><strong data-no-i18n>${escapeHtml(thread.cwd || t('未记录'))}</strong></div>
    ${source ? `<div class="detail-row"><span>来源</span><strong data-no-i18n>${escapeHtml(source)}</strong></div>` : ''}
    ${thread.cliVersion ? `<div class="detail-row"><span>CLI 版本</span><strong data-no-i18n>${escapeHtml(thread.cliVersion)}</strong></div>` : ''}
    ${thread.forkedFromId ? `<div class="detail-row"><span>Fork 来源</span><strong data-no-i18n>${escapeHtml(thread.forkedFromId)}</strong></div>` : ''}
    ${thread.parentThreadId ? `<div class="detail-row"><span>父会话</span><strong data-no-i18n>${escapeHtml(thread.parentThreadId)}</strong></div>` : ''}`
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
    toast(t('会话信息已保存'))
  } catch (error) {
    if (hadExisting) state.openingMessages[key] = existing
    else delete state.openingMessages[key]
    showError(error)
  }
}

async function generateSessionResponsibility() {
  if (!state.selectedId || isRouterThread()) return
  if (state.model.activeTurnId) throw new Error(t('请等待当前 Turn 完成后再生成会话职责。'))
  const ref = { backend: state.backend, id: state.selectedId }
  const key = sessionRefKey(ref.backend, ref.id)
  const button = $('#generate-session-responsibility')
  button.disabled = true
  button.textContent = t('正在生成…')
  const opening = $('#session-opening-question')?.value.trim() || state.openingMessages[key]?.text || ''
  const prompt = `Generate a concise session summary that can be used as this session's responsibility for routing future user queries. Describe the domain this session owns and the kinds of requests it should receive. Use the session conversation as primary context.${opening ? ` The opening question is:\n${opening}` : ''}\nReturn only the responsibility summary, with no preface or formatting.`
  try {
    const result = await sessionDispatch.startTurn(ref, [{ type: 'text', text: prompt }], {
      turnOptions: configuredTurnOptions(),
      timeoutMs: 60_000,
    })
    if (!result?.turn?.id) throw new Error(t('无法启动会话职责生成。'))
    applyCodexNotification(state.model, { method: 'turn/started', params: { threadId: ref.id, turn: result.turn } })
    cacheThreadModel(ref.backend, ref.id, state.model)
    renderTranscript()
    renderComposerState()
    const turn = await waitForSessionTurn(ref, result.turn.id, 300_000)
    const responsibility = truncateCharacters(finalAgentText(turn).trim(), 4096)
    if (!responsibility) throw new Error(t('AI 没有返回会话职责摘要。'))
    if ($('#thread-info-dialog').open && selectedStateKey() === key) $('#session-responsibility').value = responsibility
    toast(t('会话职责已生成，请确认后保存'))
  } finally {
    if (button.isConnected) {
      button.disabled = false
      button.textContent = t('AI 生成')
    }
  }
}

async function waitForSessionTurn(ref, turnId, timeoutMs) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const model = await ensureSessionModel(ref)
    const turn = model.turns?.find((candidate) => String(candidate.id) === String(turnId))
    if (turn && turn.status !== 'inProgress' && model.status !== 'running') {
      if (turn.status === 'failed') throw new Error(turn.error?.message || t('会话职责生成失败。'))
      if (state.backend === ref.backend && state.selectedId === ref.id) {
        renderTranscript()
        renderComposerState()
      }
      return turn
    }
    await new Promise((resolve) => setTimeout(resolve, 900))
  }
  throw new Error(t('会话职责生成超时。'))
}

async function copyOpeningMessage() {
  const text = state.openingMessages[selectedStateKey()]?.text
  if (!text) return
  await navigator.clipboard.writeText(text)
  toast('起始问题已复制')
}

function renderWorkspace() {
  const thread = selectedThread()
  const hasThread = Boolean(thread)
  $('#thread-heading').classList.toggle('hidden', !hasThread)
  $('#thread-actions').classList.toggle('hidden', !hasThread)
  $('#empty-workspace').classList.toggle('hidden', hasThread)
  $('#native-workspace').classList.toggle('hidden', !hasThread)
  workspaceTools.sync(thread)
  if (!thread) {
    renderSessionMap()
    return
  }
  $('#thread-title').textContent = threadTitle(thread)
  $('#thread-path').textContent = thread.cwd || thread.id
  $('#archive-thread').disabled = state.backend === 'opencode'
  $('#archive-thread').title = t(state.backend === 'opencode' ? 'OpenCode 后端暂不支持归档' : '归档会话')
  $('#router-settings-action').classList.toggle('hidden', !isRouterThread())
  renderProjectEnvironmentEntry()
  renderComposerState()
  renderAnnotationRail()
  captureOpeningMessage()
  renderSessionFavoriteCount()
  renderSessionMap()
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
    toast('Map 已创建')
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
  $('#session-map-action span').textContent = t(hasMap ? '打开 Map' : '创建 Map')
  const anotherDockIsOpen = (state.artifact && !$('#artifact-rail').classList.contains('hidden'))
    || !$('#annotation-rail').classList.contains('hidden')
    || !$('#favorites-rail').classList.contains('hidden')
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
    ? t('AI 正在生成初始结构…')
    : map.backend === 'codex'
      ? t('还没有项目。可以用 AI 生成，或手动添加。')
      : t('还没有项目，请手动添加第一项。')
  tree.innerHTML = items.length
    ? flattenSessionMap(map).map(({ item, depth }) => renderSessionMapRow(item, depth, map)).join('')
    : `<div class="session-map-empty"><span>⌁</span><strong>${t('Map 还是空的')}</strong><p>${emptyDescription}</p><div class="session-map-empty-actions">${map.backend === 'codex' ? `<button class="subtle-button" type="button" data-map-empty-ai${emptySync?.state === 'syncing' ? ' disabled' : ''}>${t('AI 生成')}</button>` : ''}<button class="subtle-button" type="button" data-map-empty-add>${t('添加')}</button></div></div>`

  const progress = mapProgress(map)
  $('#session-map-progress').textContent = t('{explored}/{total} 已浏览 · {done} 完成', progress)
  $('#session-map-revision').textContent = `rev ${map.revision}`
  $('#session-map-ai-generate').textContent = items.length ? t('AI 补全') : t('AI 生成')
  $('#session-map-ai-generate').disabled = emptySync?.state === 'syncing'
  const sync = state.sessionMapSync.get(key) || (map.backend === 'codex'
    ? { state: 'synced', message: '等待下一次对话' }
    : { state: '', message: 'OpenCode Map 当前由用户维护' })
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
    <button class="session-map-row-menu" type="button" data-map-item-menu="${escapeHtml(item.id)}" aria-label="项目操作">•••</button>
  </div>`
}

function mapStateLabel(value) {
  return ({ notStarted: '未开始', active: '当前', visited: '已浏览', done: '完成', paused: '暂停' })[value] || value
}

function setSessionMapSyncState(value, message = '') {
  const element = $('#session-map-sync-state')
  element.className = `session-map-sync-state ${value || ''}`
  element.title = message || 'Map 同步状态'
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
    if (!window.confirm(t('移除“{title}”及其子项？你可以立即撤销。', { title: item.title }))) return
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
  $('#session-map-item-dialog-title').textContent = item ? t('编辑项目') : t('添加项目')
  $('#session-map-item-id').value = item?.id || ''
  $('#session-map-item-title').value = item?.title || ''
  $('#session-map-item-kind').value = item?.kind || 'item'
  $('#session-map-item-summary').value = item?.summary || ''
  const parent = $('#session-map-item-parent')
  parent.innerHTML = `<option value="">${t('顶层')}</option>${visibleMapItems(map)
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
  if (state.backend !== 'codex') throw new Error('OpenCode 会话暂不支持 AI 重新生成目标')
  const button = $('#session-map-suggest-goal')
  const original = button.textContent
  button.disabled = true
  button.textContent = t('正在生成…')
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
      timeoutMessage: 'AI 生成目标超时',
    })
    if (!result?.goal) throw new Error('AI 没有返回可用目标')
    $('#session-map-goal-input').value = result.goal
    $('#session-map-done-input').value = result.definitionOfDone || ''
    toast('AI 建议已填入，请确认后保存')
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
  toast('已撤销最近一次 Map 更新')
}

async function deleteSessionMap() {
  closeActionMenus()
  const key = selectedStateKey()
  const map = selectedSessionMap()
  if (!key || !map || !window.confirm(t('删除这个会话的 Map？聊天记录不会受影响。'))) return
  await sessionMapFetch(sessionMapEndpoint(map.backend, map.threadId), { method: 'DELETE' })
  if (map.backend === 'codex' && state.backend === 'codex' && state.ready) {
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
  toast('Map 已删除')
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
  if (backend !== 'codex') {
    if (automatic) return null
    throw new Error('OpenCode 会话暂不支持 AI 生成 Map')
  }
  if (automatic && state.sessionMapBootstrapAttempts.has(key)) return map
  if (automatic) state.sessionMapBootstrapAttempts.add(key)

  const interactions = (model?.turns || []).map((turn) => ({
    user: questionForTurn(turn).trim(),
    assistant: answerForMapTurn(turn),
  })).filter((interaction) => interaction.user || interaction.assistant)
  const sourceTurn = [...(model?.turns || [])].reverse().find((turn) => turn?.id && (questionForTurn(turn).trim() || answerForMapTurn(turn)))
  state.sessionMapSync.set(key, { state: 'syncing', message: 'AI 正在生成初始 Map' })
  if (selectedStateKey() === key) renderSessionMap()

  try {
    const result = await runCodexStructuredWorker({
      key,
      developerInstructions: 'You create a compact navigation Map for another conversation. Do not use tools, inspect files, or answer the user. Return only the JSON object required by the supplied output schema. Follow the safe-operation restrictions exactly.',
      input: bootstrapMapInput(map, interactions),
      outputSchema: assistantOperationSchema(),
      timeoutMessage: 'AI 生成 Map 超时',
    })
    const operations = safeAssistantOperations(result)
    if (!operations.length) throw new Error('AI 没有生成可用的 Map 项目')
    const updated = await applySessionMapOperations(operations, {
      actor: 'assistant',
      sourceTurnId: sourceTurn?.id ? String(sourceTurn.id) : null,
      key,
    })
    state.sessionMapSync.set(key, { state: 'synced', message: 'Map 结构已更新' })
    if (selectedStateKey() === key) renderSessionMap()
    return updated
  } catch (error) {
    state.sessionMapSync.set(key, { state: 'error', message: error.message })
    if (selectedStateKey() === key) renderSessionMap()
    throw error
  }
}

async function processSessionMapInlineUpdate(backend, threadId, model, completedTurnId = null) {
  if (backend !== 'codex' || !threadId || !model) return
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
    if (!message) throw new Error(t('本轮回复未包含 Map 更新区块；可以手动执行 AI 补全'))
    const parsed = parseSessionMapUpdate(message.text)
    if (!parsed.found) throw new Error(t('本轮回复未包含 Map 更新区块'))
    if (parsed.update.baseRevision !== map.revision) {
      throw new Error(t('Map 更新版本过期：回复基于 rev {responseRevision}，当前为 rev {currentRevision}', {
        responseRevision: parsed.update.baseRevision,
        currentRevision: map.revision,
      }))
    }
    if (parsed.update.operations.length > 40) throw new Error(t('Map 更新操作超过 40 条限制'))
    const operations = safeAssistantOperations(parsed.update)
    if (operations.length !== parsed.update.operations.length) throw new Error(t('Map 更新包含不安全或未知操作'))
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
      message: operations.length ? t('Map 已从本轮回复更新') : t('本轮回复不需要调整 Map'),
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
    if (!worker.threadId || !state.ready || state.backend !== 'codex') return
    rpc('thread/delete', { threadId: worker.threadId }).catch((error) => {
      console.warn('Unable to release ephemeral Session Map worker', error)
    })
  })
}

function runCodexStructuredWorkerTurn(worker, { developerInstructions, input, outputSchema, timeoutMessage }) {
  return new Promise((resolve, reject) => {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/codex`)
    const pending = new Map()
    const buffered = []
    const hiddenModel = createCodexViewModel()
    let hiddenThreadId = worker.threadId
    let hiddenTurnId = null
    let serverGeneration = null
    let started = false
    let settled = false
    const timeout = setTimeout(() => finish(new Error(timeoutMessage || 'Codex 结构化任务超时')), 150_000)

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
        state.hiddenCodexThreads.add(String(hiddenThreadId))
        hiddenModel.threadId = hiddenThreadId
        const result = await request('turn/start', {
          threadId: hiddenThreadId,
          input: [{ type: 'text', text: `Task-specific instructions:\n${developerInstructions}\n\nAuthoritative task input:\n${input}` }],
          outputSchema,
        })
        hiddenTurnId = result?.turn?.id
        if (!hiddenTurnId) throw new Error('Codex did not start the Map reconciliation turn')
        state.hiddenCodexTurns.add(String(hiddenTurnId))
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
            state.hiddenCodexThreads.delete(String(staleThreadId))
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
    socket.onerror = () => finish(new Error('无法连接 Codex Map 同步服务'))
    socket.onclose = () => finish(new Error('Codex Map 同步连接已关闭'))
  })
}

function parseStructuredJson(value) {
  const text = String(value || '').trim()
  if (!text) throw new Error('结构化 AI 任务没有返回结果')
  const unwrapped = text.startsWith('```')
    ? text.replace(/^```(?:json)?\s*/u, '').replace(/\s*```$/u, '')
    : text
  return JSON.parse(unwrapped)
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

function renderTranscript({ preserveScroll = false, previousHeight = 0, previousTop = 0 } = {}) {
  if (!state.selectedId) return
  resetStreamingPatches()
  const container = $('#transcript')
  const turns = state.model.turns || []
  const entry = currentPresentationEntry()
  const visibleIds = entry.orderedIds.slice(entry.visibleStart)
  const turnById = new Map(turns.map((turn) => [String(turn.id || ''), turn]))
  const older = entry.visibleStart > 0
    ? `<button class="load-earlier-turns" type="button" data-load-earlier>${t('更早的 {count} 个 Turn', { count: entry.visibleStart })}</button>`
    : ''
  container.innerHTML = older + visibleIds.map((id) => {
    const turn = turnById.get(id)
    if (!turn) return ''
    const index = entry.orderedIds.indexOf(id)
    if (isRouterThread()) return renderRouterTurn(turn, index)
    return renderTurn(entry.turns.get(id)?.presentation, index)
  }).join('') + renderApprovals()
  bindApprovalButtons()
  bindActivityDetails()
  renderTurnNavigator()
  if (preserveScroll) container.scrollTop = previousTop + Math.max(0, container.scrollHeight - previousHeight)
  else followTranscriptOutput()
  captureOpeningMessage()
}

function handleTranscriptScroll() {
  const transcript = $('#transcript')
  transcriptScrollFollower.handleScroll(transcript)
  if (state.selectedId) transcriptPresentationCache.setScrollTop(presentationThreadKey(), transcript.scrollTop)
  scheduleTurnNavigatorSync()
}

function followTranscriptOutput() {
  if (!transcriptScrollFollower.following) return
  const transcript = $('#transcript')
  transcript.scrollTop = transcript.scrollHeight
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
    const title = turnPromptPreview(turn) || t('用户输入 {index}', { index: index + 1 })
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
    const activity = renderedActivity(turnId)
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
}

function renderedActivity(turnId) {
  return [...$('#transcript').querySelectorAll('.work-activity[data-turn-id]')]
    .find((element) => element.dataset.turnId === String(turnId || '')) || null
}

function replaceRenderedTurn(turnId, { preserveActivity = true } = {}) {
  const section = [...$('#transcript').querySelectorAll('.turn[data-turn-id]')]
    .find((element) => element.dataset.turnId === String(turnId || ''))
  if (!section) return false
  const openActivity = preserveActivity && Boolean(section.querySelector('.work-activity[open]'))
  const entry = transcriptPresentationCache.updateTurn(presentationThreadKey(), state.model, turnId)
  const presentation = entry.turns.get(String(turnId || ''))?.presentation
  if (!presentation) return false
  const template = document.createElement('template')
  template.innerHTML = renderTurn(presentation, entry.orderedIds.indexOf(String(turnId || '')), { openActivity })
  section.replaceWith(template.content)
  bindActivityDetails()
  if (openActivity) {
    const activity = renderedActivity(turnId)
    if (activity) hydrateActivityDetails(activity)
  }
  renderTurnNavigator()
  followTranscriptOutput()
  return true
}

function renderTurn(presentation, index, { openActivity = false } = {}) {
  if (!presentation) return ''
  const content = presentation.blocks.map((block) => renderPresentationBlock(block, presentation.id, {
    openActivity,
    forkable: isTurnForkable(presentation.source),
  })).join('')
  const placeholder = shouldShowTurnPlaceholder(presentation)
    ? `<div class="work-placeholder"><span class="message-track-mark" aria-hidden="true">${conversationTrackIcon('working')}</span><span>${t('{backend} 正在准备此 Turn…', { backend: currentBackend().name })}</span></div>`
    : ''
  return `<section class="turn" data-turn-id="${escapeHtml(presentation.id)}" data-turn-index="${index}">${content}${placeholder}</section>`
}

function renderPresentationBlock(block, turnId, options = {}) {
  if (block.type === 'user') return renderItem(block.item, turnId)
  if (block.type === 'assistant') return renderItem(block.item, turnId, { forkable: options.forkable })
  if (block.type === 'activity') return renderActivity(block, turnId, options)
  if (block.type === 'error') return `<div class="turn-error" role="alert"><strong>${t('执行失败')}</strong><span>${escapeHtml(block.message)}</span></div>`
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
  const title = block.active ? t('正在处理') : t('工作过程')
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
  if (explored) parts.push(t('探索 {count} 项', { count: explored }))
  if (summary.commands) parts.push(t('运行 {count} 个命令', { count: summary.commands }))
  if (summary.tools) parts.push(t('调用 {count} 个工具', { count: summary.tools }))
  if (summary.webSearches) parts.push(t('搜索网页 {count} 次', { count: summary.webSearches }))
  if (summary.changedFiles) parts.push(t('修改 {count} 个文件', { count: summary.changedFiles }))
  if (summary.failures) parts.push(t('{count} 项失败', { count: summary.failures }))
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

function activityBlockForTurn(turnId) {
  const presentation = transcriptPresentationCache.updateTurn(presentationThreadKey(), state.model, turnId)
    .turns.get(String(turnId || ''))?.presentation
  return presentation?.blocks.find((block) => block.type === 'activity') || null
}

function hydrateActivityDetails(details, { force = false } = {}) {
  const body = details.querySelector('.activity-detail-body')
  if (!body || (!force && body.dataset.activityEmpty !== 'true')) return
  const block = activityBlockForTurn(details.dataset.turnId)
  if (!block) return
  body.innerHTML = (block.displayEntries || block.entries).map(renderActivityEntry).join('')
    + `<button class="activity-log-button" type="button" data-activity-log="${escapeHtml(details.dataset.turnId)}">${t('查看完整活动记录')}</button>`
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
    return `<div class="activity-entry command-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('运行')}</strong><code>${escapeHtml(command)}</code></header>${renderOutputPreview(preview)}</div>`
  }
  if (entry.kind === 'change') {
    const rows = (item.changes || []).map((change) => `<li><span>${escapeHtml(change.kind || 'update')}</span><code>${escapeHtml(change.path || '')}</code></li>`).join('')
    return `<div class="activity-entry change-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('文件修改 · {count} 个文件', { count: (item.changes || []).length })}</strong></header><ul>${rows}</ul></div>`
  }
  if (entry.kind === 'plan') {
    const rows = (item.plan || []).map((step) => `<li class="${escapeHtml(step.status || '')}">${escapeHtml(step.step || '')}</li>`).join('')
    return `<div class="activity-entry plan-entry"><header><span>☷</span><strong>${t('执行计划')}</strong></header><ol class="plan-list">${rows}</ol></div>`
  }
  if (entry.kind === 'search') {
    return `<div class="activity-entry tool-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${t('网页搜索')}</strong><span>${escapeHtml(item.query || '')}</span></header></div>`
  }
  if (entry.kind === 'tool') {
    const label = `${item.server || 'Tool'} · ${item.tool || item.type || 'tool'}`
    const preview = activityOutputPreview(valueText(item.result || item.error || ''))
    return `<div class="activity-entry tool-entry ${status}"><header><span>${activityEntryIcon(item.status)}</span><strong>${escapeHtml(label)}</strong></header>${renderOutputPreview(preview)}</div>`
  }
  if (entry.kind === 'system') return `<div class="activity-entry system-entry"><span>•</span><p>${item.type === 'contextCompaction' ? t('Codex 已压缩较早的会话上下文。') : escapeHtml(item.reason || item.type || '')}</p></div>`
  return `<div class="activity-entry unknown-entry"><span>•</span><p>${escapeHtml(item.type || t('未知'))}</p></div>`
}

function activityEntryIcon(status) {
  if (status === 'failed') return '×'
  if (status === 'inProgress') return '<i class="activity-spinner"></i>'
  return '✓'
}

function renderOutputPreview(preview) {
  if (!preview?.lines?.length) return ''
  const lines = [...preview.lines]
  if (preview.omitted && preview.splitAt != null) lines.splice(preview.splitAt, 0, t('… 省略 {count} 行', { count: preview.omitted }))
  return `<pre>${escapeHtml(lines.join('\n'))}</pre>`
}

function renderRouterTurn(turn, index) {
  const items = Array.isArray(turn.items) ? turn.items : []
  const userItems = items.filter((item) => item.type === 'userMessage').map((item) => renderItem(item, turn.id)).join('')
  const runtimeKey = routerRuntimeKey(state.backend, turn.id)
  const runtime = state.routerDispatches.get(runtimeKey)
  const decision = runtime?.decision || routerDecisionForTurn(turn, currentRouterCandidates().map((candidate) => candidate.key))
  let card = ''
  if (decision?.action === 'clarify') {
    card = `<article class="router-card clarify"><header><span class="router-card-mark">?</span><div><strong>${t('需要确认目标')}</strong><small>${escapeHtml(decision.reason || '')}</small></div></header><p>${escapeHtml(decision.message)}</p></article>`
  } else if (decision?.action === 'dispatch') {
    const targetRef = parseSessionRefKey(decision.targetSessionKey)
    const target = targetRef && state.threadsByBackend[targetRef.backend]?.find((thread) => thread.id === targetRef.id)
    const status = runtime?.status || 'routed'
    const labels = {
      dispatching: '正在派发', running: '目标执行中', completed: '目标已完成', failed: '派发失败', routed: '已路由',
    }
    const targetLabel = target ? threadTitle(target) : decision.targetSessionKey
    const footerLabel = status === 'completed' ? t('目标响应已完成') : t('请求已发送到目标会话')
    const linkLabel = status === 'completed' ? t('打开响应') : t('打开会话')
    card = `<article class="router-card ${escapeHtml(status)}"><header><span class="router-card-mark">→</span><div><strong>${escapeHtml(targetLabel)}</strong><small>${escapeHtml(decision.reason || '')}</small></div><span class="router-card-status">${t(labels[status] || labels.routed)}</span></header>${runtime?.error ? `<p class="router-card-error">${escapeHtml(runtime.error)}</p>` : ''}<footer><span>${footerLabel}</span><button type="button" data-router-target="${escapeHtml(targetRef?.id || '')}" data-router-backend="${escapeHtml(targetRef?.backend || '')}" data-router-turn="${escapeHtml(runtime?.targetTurnId || '')}">${linkLabel}</button></footer></article>`
  } else if (runtime?.status === 'failed') {
    card = `<article class="router-card failed"><header><span class="router-card-mark">!</span><div><strong>${t(runtime.decisionInvalid ? 'Router 决策无效' : '路由失败')}</strong><small>${escapeHtml(runtime.error || '')}</small></div></header>${runtime.decisionInvalid ? renderRouterDecisionDebug(turn) : ''}</article>`
  } else if (turn.status === 'inProgress' || state.routerPending.has(runtimeKey) || ['routing', 'dispatching'].includes(runtime?.status)) {
    card = `<article class="router-card routing"><header><span class="router-card-mark pulse-mark">↝</span><div><strong>${t('正在选择目标会话')}</strong><small>${t('Router 正在比较会话职责')}</small></div></header></article>`
  } else {
    card = `<article class="router-card failed"><header><span class="router-card-mark">!</span><div><strong>${t('Router 决策无效')}</strong><small>${t('Router 没有返回候选列表中的有效目标会话。')}</small></div></header>${renderRouterDecisionDebug(turn)}</article>`
  }
  return `<section class="turn router-turn" data-turn-id="${escapeHtml(turn.id || '')}"><div class="turn-separator">Turn ${index + 1}</div>${userItems}${card}</section>`
}

function renderRouterDecisionDebug(turn) {
  const raw = finalAgentText(turn).slice(0, 16 * 1024)
  return raw ? `<details class="router-decision-debug"><summary>${t('查看原始决策')}</summary><pre>${escapeHtml(raw)}</pre></details>` : ''
}

function renderItem(item, turnId, { forkable = false } = {}) {
  const type = item?.type || 'unknown'
  const attrs = `data-turn-id="${escapeHtml(turnId || '')}" data-item-id="${escapeHtml(item?.id || '')}"`
  if (type === 'userMessage') {
    return `<div class="message user" ${attrs}><span class="message-track-mark user-track-mark" aria-hidden="true">${conversationTrackIcon('question')}</span><div class="message-content">${escapeHtml(textFromUserContent(item.content) || t('(非文字输入)'))}</div></div>`
  }
  if (type === 'agentMessage' || type === 'plan') {
    const favorite = favoriteForSource(state.backend, state.selectedId, turnId, item.id)
    const favoriteLabel = favorite ? '已收藏，点击查看' : '收藏这条回复'
    const forkAction = forkable
      ? `<button class="message-fork-button" type="button" data-fork-turn="${escapeHtml(turnId || '')}" title="${t('从这里 Fork')}" aria-label="${t('从这里 Fork')}"><svg viewBox="0 0 18 18" aria-hidden="true"><circle cx="4.25" cy="4" r="1.65"></circle><circle cx="4.25" cy="14" r="1.65"></circle><circle cx="13.75" cy="9" r="1.65"></circle><path d="M4.25 5.65v6.7M5.9 4h2.15a4.05 4.05 0 0 1 4.05 4.05V9"></path></svg><b>${t('从这里 Fork')}</b></button>`
      : ''
    return `<div class="message agent${favorite ? ' favorited' : ''}" ${attrs}>
      <span class="message-track-mark agent-track-mark" aria-hidden="true">${conversationTrackIcon('response')}</span>
      <div class="message-content"><div class="markdown-body">${renderMarkdown(type === 'agentMessage' ? sessionMapVisibleText(item.text) : item.text || '')}</div>
      <div class="message-actions"><button class="message-copy-button" type="button" data-copy-message="${escapeHtml(item.id || '')}" title="${t('复制内容')}" aria-label="${t('复制内容')}"><svg viewBox="0 0 18 18" aria-hidden="true"><rect x="2.75" y="2.75" width="8.5" height="10" rx="1.5"></rect><rect x="6.75" y="5.25" width="8.5" height="10" rx="1.5"></rect></svg><b>${t('复制')}</b></button><button class="message-favorite-button${favorite ? ' active' : ''}" type="button" data-favorite-message="${escapeHtml(item.id || '')}" title="${favoriteLabel}" aria-label="${favoriteLabel}" aria-pressed="${Boolean(favorite)}"><svg viewBox="0 0 18 18" aria-hidden="true"><path d="m9 2.8 2.02 4.09 4.51.66-3.27 3.18.77 4.5L9 13.11l-4.03 2.12.77-4.5-3.27-3.18 4.51-.66Z"></path></svg><b>${favorite ? '已收藏' : '收藏'}</b></button>${forkAction}</div></div>
    </div>`
  }
  if (type === 'reasoning') {
    const summary = arrayText(item.summary) || arrayText(item.content) || t('{backend} 正在推理…', { backend: currentBackend().name })
    return `<details class="reasoning" ${attrs} open><summary>推理摘要</summary><div class="markdown-body compact-markdown">${renderMarkdown(summary)}</div></details>`
  }
  if (type === 'commandExecution') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    return `<article class="item-card" ${attrs}><header><span>${t('命令')} · ${escapeHtml(command)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${item.aggregatedOutput ? `<pre>${escapeHtml(item.aggregatedOutput)}</pre>` : ''}</article>`
  }
  if (type === 'fileChange') {
    const changes = (item.changes || []).map((change) => `${change.kind || 'update'} ${change.path || ''}\n${change.diff || ''}`).join('\n\n')
    return `<article class="item-card" ${attrs}><header><span>${t('文件修改 · {count} 个文件', { count: (item.changes || []).length })}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header><pre>${escapeHtml(changes || t('等待差异内容…'))}</pre></article>`
  }
  if (type === 'planUpdate') {
    const rows = (item.plan || []).map((step) => `<li class="${escapeHtml(step.status || '')}">${escapeHtml(step.step || '')}</li>`).join('')
    return `<article class="item-card" ${attrs}><header><span>执行计划</span><span data-no-i18n>${escapeHtml(item.explanation || '')}</span></header><ol class="plan-list" data-no-i18n>${rows}</ol></article>`
  }
  if (type === 'mcpToolCall' || type === 'collabToolCall' || type === 'webSearch') {
    const label = type === 'webSearch' ? `${t('网页搜索')} · ${item.query || ''}` : `${item.server || 'Tool'} · ${item.tool || type}`
    const detail = item.result || item.error || item.arguments || item.results || ''
    return `<article class="item-card" ${attrs}><header><span>${escapeHtml(label)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${detail ? `<pre>${escapeHtml(valueText(detail))}</pre>` : ''}</article>`
  }
  if (type === 'contextCompaction') return `<div class="reasoning" ${attrs}>Codex 已压缩较早的会话上下文。</div>`
  return `<article class="item-card" ${attrs}><header><span>${escapeHtml(type)}</span></header><pre>${escapeHtml(valueText(item))}</pre></article>`
}

function renderMarkdown(value) {
  const source = String(value || '')
  if (!source) return ''
  const cacheKey = `${getLocale()}\u0000${source}`
  const cached = markdownRenderCache.get(cacheKey)
  if (cached != null) return cached
  const dirty = marked.parse(source)
  const clean = DOMPurify.sanitize(dirty, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['button', 'form', 'iframe', 'object', 'embed', 'script', 'style'],
    FORBID_ATTR: ['style'],
  })
  const template = document.createElement('template')
  template.innerHTML = clean

  template.content.querySelectorAll('a').forEach((link) => {
    link.target = '_blank'
    link.rel = 'noopener noreferrer'
  })
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
    copy.textContent = t('复制')
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
      sourceToggle.textContent = t('源码')
      actions.append(sourceToggle, copy)
      header.append(label, actions)
      const canvas = document.createElement('div')
      canvas.className = 'markdown-mermaid-canvas'
      canvas.dataset.noI18n = ''
      canvas.setAttribute('role', 'img')
      canvas.setAttribute('aria-label', t('Mermaid 图表'))
      canvas.setAttribute('aria-busy', 'true')
      canvas.textContent = t('正在渲染图表…')
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
  const rendered = template.innerHTML
  if (source.length <= 64_000) {
    markdownRenderCache.set(cacheKey, rendered)
    if (markdownRenderCache.size > 256) markdownRenderCache.delete(markdownRenderCache.keys().next().value)
  }
  return rendered
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
    showMermaidError(block, t('图表内容过大，已显示源码。'))
    return
  }
  const mermaid = globalThis.mermaid
  if (!mermaid?.initialize || !mermaid?.render) {
    showMermaidError(block, t('图表无法渲染'))
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
  canvas.textContent = t('正在渲染图表…')
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
    showMermaidError(block, t('图表无法渲染'))
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
  button.textContent = t(visible ? '隐藏源码' : '源码')
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
      canvas.textContent = t('正在渲染图表…')
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
    button.textContent = t('已复制')
    setTimeout(() => { if (button.isConnected) button.textContent = original }, 1400)
  } catch {
    toast(t('无法复制代码'), 'error')
  }
}

async function handleTranscriptClick(event) {
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
    await selectThread(routerTarget.dataset.routerTarget, { backend: routerTarget.dataset.routerBackend })
    const turnId = routerTarget.dataset.routerTurn
    if (turnId) {
      requestAnimationFrame(() => document.querySelector(`[data-turn-id="${CSS.escape(turnId)}"]`)?.scrollIntoView({ block: 'start' }))
    }
    return
  }
  const forkButton = event.target.closest('[data-fork-turn]')
  if (forkButton) {
    await forkThread(forkButton.dataset.forkTurn, forkButton)
    return
  }
  const favoriteButton = event.target.closest('[data-favorite-message]')
  if (favoriteButton) {
    const element = favoriteButton.closest('[data-turn-id][data-item-id]')
    if (!element) return
    const existing = favoriteForSource(
      state.backend,
      state.selectedId,
      element.dataset.turnId,
      element.dataset.itemId,
    )
    if (existing) await openFavoriteDetail(existing.id)
    else openFavoriteForMessage(element.dataset.turnId, element.dataset.itemId)
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
      toast(t('已复制'))
      setTimeout(() => { if (copyMessageButton.isConnected) copyMessageButton.classList.remove('copied') }, 1400)
    } catch {
      toast(t('无法复制回复'), 'error')
    }
    return
  }
}

function openActivityLog(turnId) {
  const block = activityBlockForTurn(turnId)
  if (!block) return
  activityLogContext = { turnId: String(turnId || ''), block }
  $('#activity-log-title').textContent = t('完整活动记录')
  $('#activity-log-subtitle').textContent = t('原始详情按项目加载，不会影响主聊天流。')
  $('#activity-log-content').innerHTML = block.entries.map((entry, index) => {
    const item = entry.item || {}
    const label = activityRawLabel(entry)
    return `<details class="activity-raw-item" data-activity-entry-index="${index}"><summary><span>${activityEntryIcon(item.status)}</span><strong>${escapeHtml(label)}</strong><small>${escapeHtml(statusLabel(item.status || entry.status))}</small></summary><div class="activity-raw-body" data-raw-empty="true"></div></details>`
  }).join('') || `<div class="command-empty">${t('没有活动记录')}</div>`
  $$('#activity-log-content .activity-raw-item').forEach((details) => details.addEventListener('toggle', () => {
    if (!details.open) return
    const body = details.querySelector('.activity-raw-body')
    if (!body || body.dataset.rawEmpty !== 'true') return
    const entry = activityLogContext?.block.entries[Number(details.dataset.activityEntryIndex)]
    if (!entry) return
    body.innerHTML = renderRawActivityEntry(entry)
    body.dataset.rawEmpty = 'false'
  }))
  $('#activity-log-dialog').showModal()
}

function activityRawLabel(entry) {
  const item = entry.item || {}
  if (entry.kind === 'command') return Array.isArray(item.command) ? item.command.join(' ') : item.command || t('命令')
  if (entry.kind === 'reasoning') return reasoningStage(item) || t('推理摘要')
  if (entry.kind === 'progress') return truncateForDisplay(item.text || '', 100)
  if (entry.kind === 'change') return t('文件修改 · {count} 个文件', { count: item.changes?.length || 0 })
  if (entry.kind === 'search') return `${t('网页搜索')} · ${item.query || ''}`
  if (entry.kind === 'tool') return `${item.server || 'Tool'} · ${item.tool || item.type || 'tool'}`
  if (entry.kind === 'plan') return t('执行计划')
  return item.type || t('未知')
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
      <strong>${permission ? t('{backend} 请求额外权限', { backend: currentBackend().name }) : t('{backend} 正在等待审批', { backend: currentBackend().name })}</strong>
      <pre>${escapeHtml(command)}${params.cwd ? `\n${escapeHtml(params.cwd)}` : ''}</pre>
      <div class="approval-actions">
        <button class="subtle-button approval-decline" type="button">拒绝</button>
        <button class="subtle-button approval-session" type="button">本会话允许</button>
        <button class="primary-button approval-accept" type="button">允许本次</button>
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
        <strong>${t('Codex 正在等待你的输入')}</strong>
        <div class="interaction-fields">${questions}</div>
        <div class="approval-actions"><button class="subtle-button interaction-cancel" type="button">${t('取消')}</button><button class="primary-button interaction-submit" type="button">${t('提交')}</button></div>
      </article></section>`
    }
    const mode = params.mode || 'form'
    const schema = params.requestedSchema || {}
    const fields = mode === 'url' ? '' : renderElicitationSchema(schema)
    return `<section class="turn"><article class="approval-card interaction-card" data-interaction-id="${escapeHtml(String(interaction.id))}" data-interaction-method="${escapeHtml(interaction.method)}" data-url="${escapeHtml(params.url || '')}">
      <strong>${escapeHtml(params.serverName || 'MCP')} ${t('正在等待你的输入')}</strong>
      <p>${escapeHtml(params.message || '')}</p>
      <div class="interaction-fields">${fields}</div>
      <div class="approval-actions"><button class="subtle-button interaction-decline" type="button">${t('拒绝')}</button>${mode === 'url' ? `<button class="subtle-button interaction-open-url" type="button">${t('打开链接')}</button>` : ''}<button class="primary-button interaction-submit" type="button">${t(mode === 'url' ? '已完成' : '提交')}</button></div>
    </article></section>`
  }).join('')
}

function renderUserInputQuestion(question = {}) {
  const name = `interaction-${String(question.id || randomId()).replace(/[^a-z0-9_-]/giu, '-')}`
  const options = Array.isArray(question.options) ? question.options : []
  const inputType = question.isSecret ? 'password' : 'text'
  const choices = options.map((option, index) => `<label class="interaction-option"><input type="radio" name="${escapeHtml(name)}" value="${escapeHtml(option.label || '')}" ${index === 0 ? 'checked' : ''}/><span><strong>${escapeHtml(option.label || '')}</strong><small>${escapeHtml(option.description || '')}</small></span></label>`).join('')
  const freeform = !options.length || question.isOther
    ? `<input class="interaction-freeform" data-question-id="${escapeHtml(question.id || '')}" type="${inputType}" placeholder="${escapeHtml(question.isOther ? t('其他…') : t('请输入…'))}" autocomplete="${question.isSecret ? 'off' : 'on'}" />`
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
  }).join('') || `<p>${t('此请求不需要填写额外字段。')}</p>`
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
          toast(t('请回答所有问题'), 'error')
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
    if (option && previewableFileKind(option)) openArtifact(option).catch(showError)
    else toast('此文件类型不能在文档审阅器中打开', 'error')
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
      ? '没有匹配文件'
      : state.composerMenu.type === 'skill'
        ? '没有匹配技能'
        : '没有匹配命令'
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
    const previewable = type === 'file' && Boolean(previewableFileKind(option))
    const openAction = type === 'file'
      ? `<button class="composer-file-open" type="button" data-open-file-index="${index}" title="${t(previewable ? '在审阅区打开' : '仅支持预览文本和常见图片')}"${previewable ? '' : ' disabled aria-disabled="true"'}>${t('打开')}</button>`
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
  renderComposerMenu('正在由 Codex App Server 搜索文件…')
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
    renderComposerMenu(t('文件搜索失败：{message}', { message: error.message }))
  }
}

function searchComposerSkills(trigger) {
  const cwd = selectedThread()?.cwd || ''
  const generation = state.composerMenu.generation + 1
  state.composerMenu = { type: 'skill', trigger, options: [], selected: 0, generation }
  renderComposerMenu('正在由 Codex App Server 发现技能…')
  loadSkillCatalog(cwd).then((skills) => {
    if (generation !== state.composerMenu.generation || state.composerMenu.type !== 'skill') return
    state.composerMenu.options = matchingSkills(trigger.query, skills)
    state.composerMenu.selected = 0
    renderComposerMenu()
  }).catch((error) => {
    if (generation !== state.composerMenu.generation) return
    renderComposerMenu(t('技能读取失败：{message}', { message: error.message }))
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
    input.value = replacement.value
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
    input.value = replacement.value
    input.setSelectionRange(replacement.cursor, replacement.cursor)
    addPendingSkill(option)
    hideComposerMenu()
    renderComposerState()
    input.focus()
    return
  }
  const replacement = replaceComposerTrigger(input.value, trigger, '')
  input.value = replacement.value
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
  state.turnOptions[key] ||= {}
  return state.turnOptions[key]
}

function configuredTurnOptions(options = currentTurnOptions()) {
  const result = { ...options }
  const profile = state.environmentProfile
  if (state.backend !== 'codex' || !profile?.configured || profile.root !== selectedThread()?.cwd) return result
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
  showCommandDialog('模型', '<div class="command-empty">正在从 App Server 读取模型…</div>')
  const result = await rpc('model/list', { limit: 100, includeHidden: false })
  const models = Array.isArray(result?.data) ? result.data : []
  if (!models.length) {
    $('#command-content').innerHTML = '<div class="command-empty">没有可用模型。</div>'
    return
  }
  $('#command-content').innerHTML = `<div class="command-list">${models.map((model) => {
    const efforts = model.supportedReasoningEfforts || []
    const effortOptions = efforts.map((entry) => `<option value="${escapeHtml(entry.reasoningEffort)}"${entry.reasoningEffort === model.defaultReasoningEffort ? ' selected' : ''}>${escapeHtml(entry.reasoningEffort)}</option>`).join('')
    return `<div class="command-card"><strong>${escapeHtml(model.displayName || model.model || model.id)}</strong><small>${escapeHtml(model.model || model.id)}${model.isDefault ? t(' · 默认') : ''}</small>${effortOptions ? `<select aria-label="${t('推理强度')}">${effortOptions}</select>` : '<span></span>'}<button class="subtle-button" type="button" data-model="${escapeHtml(model.model || model.id)}">${t('使用')}</button></div>`
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
    toast(t('已选择模型 {model}{effort}', { model: options.model, effort: effort ? ` · ${effort}` : '' }))
  }
}

function openPermissionsCommand() {
  if (state.backend === 'opencode') {
    showCommandDialog('权限', '<div class="command-empty">OpenCode 权限由项目配置和运行时审批管理；收到权限请求时可允许一次、始终允许或拒绝。</div>')
    return
  }
  const choices = [
    ['readOnly', '只读', '文件只读；需要操作时由 Codex 请求批准'],
    ['workspaceWrite', '项目可写', '允许修改当前项目，网络默认关闭'],
    ['dangerFullAccess', '完全访问', '关闭沙箱限制；仅用于可信项目'],
  ]
  showCommandDialog('权限', `<div class="command-list">${choices.map(([id, title, detail]) => `<button class="command-card" type="button" data-permission="${id}"><strong>${title}</strong><small>${detail}</small><span>选择</span></button>`).join('')}</div>`)
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-permission]')
    if (!button) return
    const type = button.dataset.permission
    if (type === 'dangerFullAccess' && !confirm(t('确认对后续 Turn 使用完全访问权限？'))) return
    const options = currentTurnOptions()
    options.approvalPolicy = type === 'dangerFullAccess' ? 'never' : 'on-request'
    options.sandboxPolicy = type === 'workspaceWrite'
      ? { type, writableRoots: [selectedThread()?.cwd].filter(Boolean), networkAccess: false }
      : { type }
    $('#command-dialog').close()
    renderComposerState()
    toast('后续 Turn 权限已更新')
  }
}

function openStatusCommand() {
  const thread = selectedThread()
  const options = currentTurnOptions()
  const rows = [
    ['Thread', thread?.name || thread?.id || '—'],
    ['状态', statusLabel(state.model.status)],
    ['目录', thread?.cwd || '—'],
    ['模型', options.model || thread?.model || t('{backend} 默认', { backend: currentBackend().name })],
    ['推理强度', options.effort || t('{backend} 默认', { backend: currentBackend().name })],
    ['审批策略', options.approvalPolicy || '继承会话'],
    ['沙箱', options.sandboxPolicy?.type || '继承会话'],
    ['Token', state.model.usage ? valueText(state.model.usage) : '暂无数据'],
  ]
  showCommandDialog('会话状态', `<div class="command-summary">${rows.map(([label, value]) => `<div class="detail-row"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`).join('')}</div>`)
}

async function compactCurrentThread() {
  if (state.model.activeTurnId) throw new Error('当前 Turn 仍在运行，完成或停止后才能压缩。')
  await rpc('thread/compact/start', { threadId: state.selectedId })
  toast('Codex 已开始压缩会话上下文')
}

async function reviewCurrentChanges() {
  if (state.model.activeTurnId) throw new Error('当前 Turn 仍在运行，完成或停止后才能开始 Review。')
  transcriptScrollFollower.reset()
  const result = await rpc('review/start', { threadId: state.selectedId, target: { type: 'uncommittedChanges' }, delivery: 'inline' })
  if (result?.turn) {
    applyCodexNotification(state.model, { method: 'turn/started', params: { turn: result.turn } })
    renderTranscript()
    renderComposerState()
  }
}

function openDiffCommand() {
  showCommandDialog('当前修改', state.model.diff
    ? `<pre class="command-pre">${escapeHtml(state.model.diff)}</pre>`
    : '<div class="command-empty">当前 Turn 还没有可显示的 Diff。</div>')
}

async function openMcpCommand() {
  showCommandDialog('MCP Server', '<div class="command-empty">正在从 App Server 读取 MCP 状态…</div>')
  const result = await rpc('mcpServerStatus/list', { limit: 100 })
  const servers = Array.isArray(result?.data) ? result.data : []
  $('#command-content').innerHTML = servers.length
    ? `<div class="command-list">${servers.map((server) => `<div class="command-card"><strong>${escapeHtml(server.name)}</strong><small>${t('{tools} 个工具 · {resources} 个资源', { tools: Object.keys(server.tools || {}).length, resources: server.resources?.length || 0 })}</small><span>${escapeHtml(valueText(server.authStatus || 'unknown'))}</span></div>`).join('')}</div>`
    : '<div class="command-empty">没有配置 MCP Server。</div>'
}

async function openSkillsCommand() {
  const cwd = selectedThread()?.cwd
  showCommandDialog('技能', '<div class="command-empty">正在由 App Server 发现技能…</div>')
  const skills = await loadSkillCatalog(cwd || '')
  $('#command-content').innerHTML = skills.length
    ? `<div class="command-list">${skills.map((skill) => `<button class="command-card" type="button" data-skill-name="${escapeHtml(skill.name)}" data-skill-path="${escapeHtml(skill.path)}"><strong>$${escapeHtml(skill.name)}</strong><small>${escapeHtml(skill.description || skill.shortDescription || '')}</small><span>引用</span></button>`).join('')}</div>`
    : '<div class="command-empty">当前目录没有已启用的技能。</div>'
  $('#command-content').onclick = (event) => {
    const button = event.target.closest('[data-skill-name]')
    if (!button || !state.selectedId) return
    addPendingSkill({ name: button.dataset.skillName, path: button.dataset.skillPath })
    const input = $('#composer-input')
    input.value = `${input.value}${input.value && !input.value.endsWith(' ') ? ' ' : ''}$${button.dataset.skillName} `
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
  if (!message) throw new Error(t('当前会话还没有可复制的 {backend} 回复。', { backend: currentBackend().name }))
  await navigator.clipboard.writeText(message.type === 'agentMessage' ? sessionMapVisibleText(message.text) : message.text)
  toast(t('已复制最近一条 {backend} 回复', { backend: currentBackend().name }))
}

async function executeSlashCommand(action) {
  if (!state.selectedId && !['new'].includes(action)) throw new Error('请先选择一个 Codex 会话。')
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
  if (!handler) throw new Error(t('尚未支持命令：/{action}', { action }))
  await handler()
}

function renderComposerState() {
  const active = Boolean(state.model.activeTurnId)
  const options = currentTurnOptions()
  const shellCommand = shellCommandFromComposer($('#composer-input').value)
  const shellMode = shellCommand !== null
  $('#composer-form').classList.toggle('shell-mode', shellMode)
  $('#interrupt-turn').classList.toggle('hidden', !active)
  $('#archive-thread').disabled = active || state.backend === 'opencode'
  $('#delete-thread').disabled = active
  $('#send-message').textContent = shellMode ? t('运行命令') : isRouterThread() ? t('路由') : active && state.backend === 'codex' ? '追加意见' : '发送'
  const details = [
    options.model && `${options.model}${options.effort ? `/${options.effort}` : ''}`,
    options.sandboxPolicy?.type,
    state.pendingSkills[selectedStateKey()]?.length && t('{count} 个技能', { count: state.pendingSkills[selectedStateKey()].length }),
    state.pendingFiles[selectedStateKey()]?.length && t('{count} 个文件', { count: state.pendingFiles[selectedStateKey()].length }),
  ].filter(Boolean)
  const baseHint = isRouterThread() && !shellMode
    ? active ? 'Router 正在选择目标会话' : '请求将由 Router 派发，并在目标会话中执行'
    : shellMode
    ? active
      ? 'Shell 命令需等待当前 Turn 完成'
      : '本地 Shell · 不经过模型且不受 Turn sandbox 限制'
    : active
      ? state.backend === 'codex' ? '将通过 turn/steer 加入当前 Turn' : 'OpenCode 正在响应；完成或停止后可继续发送'
      : '将通过 turn/start 开始新 Turn'
  $('#composer-hint').textContent = `${baseHint}${!shellMode && details.length ? ` · ${details.join(' · ')}` : ''}`
  $('#send-message').disabled = !state.ready || !state.selectedId || (active && state.backend === 'opencode') || (shellMode && (active || !shellCommand))
  renderComposerReviewContext()
}

async function sendComposer(event) {
  event.preventDefault()
  const input = $('#composer-input')
  const text = input.value.trim()
  const shellCommand = shellCommandFromComposer(input.value)
  if (shellCommand !== null) {
    if (!shellCommand || !state.selectedId) return
    if (state.model.activeTurnId) {
      showError(new Error('请等待当前 Turn 完成或先停止，再运行本地 Shell 命令。'))
      return
    }
    const button = $('#send-message')
    button.disabled = true
    transcriptScrollFollower.reset()
    try {
      await rpc('thread/shellCommand', { threadId: state.selectedId, command: shellCommand }, 120_000)
      input.value = ''
      hideComposerMenu()
      renderComposerState()
      toast(t('Shell 命令已交给 {backend} 执行', { backend: currentBackend().name }))
    } catch (error) { showError(error) }
    finally { button.disabled = false }
    return
  }
  const slashName = text.match(/^\/([\w-]+)$/)?.[1]
  const slash = slashName && matchingSlashCommands(slashName).find((command) => command.name === slashName)
  if (slash) {
    input.value = ''
    hideComposerMenu()
    try {
      await executeSlashCommand(slash.action)
    } catch (error) {
      showError(error)
    }
    return
  }
  if (!text || !state.selectedId) return
  if (isRouterThread()) {
    const button = $('#send-message')
    button.disabled = true
    transcriptScrollFollower.reset()
    try {
      await startRouterTurn(text)
      input.value = ''
      hideComposerMenu()
      renderComposerState()
    } catch (error) { showError(error) }
    finally { button.disabled = false }
    return
  }
  const backend = state.backend
  const threadId = state.selectedId
  const targetModel = state.model
  const stateKey = selectedStateKey(threadId, backend)
  const skillInputs = [...(state.pendingSkills[stateKey] || [])]
  const fileInputs = [...(state.pendingFiles[stateKey] || [])]
  const turnInput = [{ type: 'text', text }, ...skillInputs, ...fileInputs]
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
      toast('意见已加入当前 Turn')
    } else {
      const clientUserMessageId = randomId()
      if (backend === 'codex') {
        optimisticTurnId = beginOptimisticCodexTurn(targetModel, { clientUserMessageId, input: turnInput })
        latencyTrace = beginTurnLatencyTrace(clientUserMessageId, threadId)
        input.value = ''
        state.pendingSkills[stateKey] = []
        state.pendingFiles[stateKey] = []
        composerCleared = true
        hideComposerMenu()
        renderComposerState()
        renderTranscript()
      }
      await prepareSessionMapTurn().catch((error) => {
        console.warn('Unable to attach Session Map context', error)
        setSessionMapSyncState('error', error.message)
      })
      const result = await rpc('turn/start', {
        threadId,
        clientUserMessageId,
        input: turnInput,
        ...turnOptions,
      })
      if (result?.turn) {
        if (backend === 'codex' && optimisticTurnId) {
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
      input.value = ''
      state.pendingSkills[stateKey] = []
      state.pendingFiles[stateKey] = []
    }
    hideComposerMenu()
    renderComposerState()
  } catch (error) {
    if (optimisticTurnId) {
      rollbackOptimisticCodexTurn(targetModel, optimisticTurnId)
      finishTurnLatencyTrace(latencyTrace, 'failed')
      if (targetModel === state.model) renderTranscript()
    }
    if (composerCleared && state.backend === backend && state.selectedId === threadId) {
      if (!input.value.trim()) input.value = text
      if (!(state.pendingSkills[stateKey] || []).length) state.pendingSkills[stateKey] = skillInputs
      if (!(state.pendingFiles[stateKey] || []).length) state.pendingFiles[stateKey] = fileInputs
      renderComposerState()
    }
    showError(error)
  }
  finally { button.disabled = false }
}

async function prepareSessionMapTurn() {
  if (state.backend !== 'codex' || !state.selectedId) return
  const map = await loadSessionMap('codex', state.selectedId)
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
  setSessionMapSyncState('syncing', '当前 Map 已加入本次 Turn 上下文')
}

function isRouterThread(threadId = state.selectedId, backend = state.backend) {
  const controller = routerControllerRef(state.router)
  return Boolean(threadId) && controller?.backend === backend && controller.id === threadId
}

function currentRouterCandidates() {
  return routerCandidates(state.router, routerTargetCatalogs(), state.openingMessages)
}

async function startRouterTurn(text) {
  if (!isRouterThread() || state.model.activeTurnId) throw new Error(t('Router 正在处理上一条请求。'))
  const controller = routerControllerRef(state.router)
  if (!controller || !sessionDispatch.supports(controller.backend)) throw new Error(t('Router 后端当前不可用。'))
  await refreshRouterCatalogs()
  const candidates = currentRouterCandidates()
  if (!candidates.length) throw new Error(t('Router 没有可用的目标会话，请先打开“路由设置”。'))
  const developerInstructions = routerDeveloperInstructions(candidates)
  const result = await sessionDispatch.startTurn(controller, [{ type: 'text', text }], {
    ...(controller.backend === 'codex'
      ? { additionalContext: routerApplicationContext(candidates) }
      : { developerInstructions }),
    outputSchema: routerDecisionSchema(candidates.map((candidate) => candidate.key)),
    turnOptions: configuredTurnOptions(),
  })
  if (!result?.turn) throw new Error(t('Router 未能启动新的 Turn。'))
  const turnId = String(result.turn.id || '')
  const runtimeKey = routerRuntimeKey(controller.backend, turnId)
  state.routerPending.set(runtimeKey, {
    candidateKeys: candidates.map((candidate) => candidate.key),
    requestedAt: Date.now(),
  })
  state.routerDispatches.set(runtimeKey, { status: 'routing' })
  applyCodexNotification(state.model, { method: 'turn/started', params: { threadId: controller.id, turn: result.turn } })
  cacheThreadModel(controller.backend, controller.id, state.model)
  renderTranscript()
  monitorRouterTurn(controller, turnId)
}

async function completeRouterTurn({ backend, turnId, model, turn: suppliedTurn }) {
  turnId = String(turnId || '')
  if (!turnId) return
  const runtimeKey = routerRuntimeKey(backend, turnId)
  const routed = state.routerTargetTurns.get(runtimeKey)
  if (routed) {
    const completed = model.turns?.find((turn) => String(turn.id) === turnId) || suppliedTurn
    const existing = state.routerDispatches.get(routed.routerTurnId) || {}
    state.routerDispatches.set(routed.routerTurnId, {
      ...existing,
      status: completed?.status === 'failed' ? 'failed' : 'completed',
      error: completed?.error?.message || '',
    })
    state.routerTargetTurns.delete(runtimeKey)
    if (isRouterThread()) renderTranscript()
    return
  }
  const pending = state.routerPending.get(runtimeKey)
  if (!pending) return
  state.routerPending.delete(runtimeKey)
  const turn = model.turns?.find((candidate) => String(candidate.id) === turnId) || suppliedTurn
  let decisionParsed = false
  try {
    const decision = parseRouterDecision(finalAgentText(turn), pending.candidateKeys)
    decisionParsed = true
    if (decision.action === 'clarify') {
      state.routerDispatches.set(runtimeKey, { status: 'clarify', decision })
      if (isRouterThread()) renderTranscript()
      return
    }
    const targetRef = parseSessionRefKey(decision.targetSessionKey)
    const target = targetRef && state.threadsByBackend[targetRef.backend]?.find((thread) => thread.id === targetRef.id)
    if (!target) throw new Error(t('目标会话已不存在。'))
    if (!sessionDispatch.supports(targetRef.backend)) throw new Error(t('目标会话的后端当前不可用。'))
    state.routerDispatches.set(runtimeKey, { status: 'dispatching', decision })
    if (isRouterThread()) renderTranscript()
    await sessionDispatch.prepareTurn(targetRef, { alreadyActive: threadStatus(target) !== 'notLoaded' })
    const targetModel = await ensureSessionModel(targetRef)
    if (targetModel.activeTurnId) throw new Error(t('“{title}”正在运行，暂时不能接收新请求。', { title: threadTitle(target) }))
    const result = await sessionDispatch.startTurn(targetRef, [{ type: 'text', text: decision.forwardedPrompt }])
    if (!result?.turn) throw new Error(t('目标会话未能启动新的 Turn。'))
    applyCodexNotification(targetModel, { method: 'turn/started', params: { threadId: targetRef.id, turn: result.turn } })
    cacheThreadModel(targetRef.backend, targetRef.id, targetModel)
    updateLoadedThreadTimestamp(targetRef.backend, targetRef.id)
    state.routerDispatches.set(runtimeKey, {
      status: 'running', decision, targetTurnId: String(result.turn.id || ''),
    })
    state.routerTargetTurns.set(routerRuntimeKey(targetRef.backend, result.turn.id), {
      routerTurnId: runtimeKey,
      targetSessionKey: targetRef.key,
    })
    renderThreadList()
    if (isRouterThread() || state.model === targetModel) {
      renderWorkspace()
      renderTranscript()
    }
    monitorRouterTurn(targetRef, result.turn.id)
  } catch (error) {
    state.routerDispatches.set(runtimeKey, { status: 'failed', error: error.message, decisionInvalid: !decisionParsed })
    if (isRouterThread()) renderTranscript()
    toast(t('路由失败：{message}', { message: error.message }), 'error')
  } finally {
    if (isRouterThread()) renderComposerState()
  }
}

function routerRuntimeKey(backend, turnId) {
  return sessionRefKey(backend, String(turnId || ''))
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

function monitorRouterTurn(ref, turnId) {
  const key = routerRuntimeKey(ref.backend, turnId)
  if (!key || state.routerMonitors.has(key)) return
  let failures = 0
  const poll = async () => {
    try {
      const model = await ensureSessionModel(ref)
      const turn = model.turns?.find((candidate) => String(candidate.id) === String(turnId))
      const terminal = turn && turn.status !== 'inProgress' && model.status !== 'running'
      if (terminal) {
        state.routerMonitors.delete(key)
        await completeRouterTurn({ backend: ref.backend, turnId, model, turn })
        return
      }
      failures = 0
    } catch (error) {
      failures += 1
      if (failures >= 5) {
        state.routerMonitors.delete(key)
        const routed = state.routerTargetTurns.get(key)
        const dispatchKey = routed?.routerTurnId || key
        state.routerDispatches.set(dispatchKey, { ...(state.routerDispatches.get(dispatchKey) || {}), status: 'failed', error: error.message })
        if (isRouterThread()) renderTranscript()
        return
      }
    }
    state.routerMonitors.set(key, setTimeout(poll, 1_000))
  }
  state.routerMonitors.set(key, setTimeout(poll, 500))
}

async function interruptTurn() {
  if (!state.selectedId || !state.model.activeTurnId) return
  try {
    await rpc('turn/interrupt', { threadId: state.selectedId, turnId: state.model.activeTurnId })
    toast('已请求停止当前 Turn')
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
  $('#new-thread-model').placeholder = unsupported ? t('可选：provider/model') : t('使用 Codex 默认模型')
  for (const id of ['new-thread-approval', 'new-thread-sandbox']) {
    const select = $(`#${id}`)
    const field = select.closest('.field')
    select.disabled = unsupported
    field.classList.toggle('capability-disabled', unsupported)
    field.querySelector('.backend-capability-note')?.classList.toggle('hidden', !unsupported)
  }
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
    toast('会话名称已保存')
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
    errorBox.textContent = t('Windows 客户端需要 WSL 中的 Linux 绝对路径。')
    errorBox.classList.remove('hidden')
    button.disabled = false
    return
  }
  const params = {
    cwd,
    ...(backend === 'codex' ? {
      approvalPolicy: $('#new-thread-approval').value,
      sandbox: $('#new-thread-sandbox').value,
    } : {}),
  }
  const model = $('#new-thread-model').value.trim()
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
    toast(t('{backend} 会话已创建', { backend: currentBackend().name }))
  } catch (error) {
    errorBox.textContent = error.message
    errorBox.classList.remove('hidden')
  } finally { button.disabled = false }
}

async function forkThread(lastTurnId = null, trigger = null) {
  const sourceThreadId = state.selectedId
  const sourceBackend = state.backend
  if (!sourceThreadId) return
  if (trigger) {
    trigger.disabled = true
    trigger.classList.add('busy')
  }
  try {
    const result = await rpc('thread/fork', threadForkParams(sourceThreadId, lastTurnId))
    await loadThreads()
    await selectThread(result.thread.id, { force: true, backend: sourceBackend })
    toast(t(lastTurnId ? '已从此 Turn 创建 {backend} 会话分支' : '已创建 {backend} 会话分支', { backend: currentBackend().name }))
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
  if (!state.selectedId || !confirm(t('归档当前 Codex 会话？'))) return
  const threadId = state.selectedId
  try {
    await rpc('thread/archive', { threadId })
    invalidateThreadModel(state.backend, threadId)
    state.selectedId = null
    state.selectedByBackend[state.backend] = null
    state.model = createCodexViewModel()
    persistPreferences()
    await loadThreads()
    toast('会话已归档')
  } catch (error) { showError(error) }
}

async function deleteSelectedThread() {
  if (!state.selectedId || !confirm(t('永久删除当前 Codex 会话及其持久化历史？此操作无法撤销。'))) return
  const threadId = state.selectedId
  try {
    await rpc('thread/delete', { threadId })
    invalidateThreadModel(state.backend, threadId)
    delete state.annotationDrafts[`${state.backend}:${threadId}`]
    delete state.annotationAdditional[`${state.backend}:${threadId}`]
    delete state.openingMessages[`${state.backend}:${threadId}`]
    state.selectedId = null
    state.selectedByBackend[state.backend] = null
    state.model = createCodexViewModel()
    persistPreferences()
    await loadThreads()
    toast('会话已删除')
  } catch (error) { showError(error) }
}

async function openArtifact(file, { allowDetachedRoot = false, returnTool = '' } = {}) {
  const thread = selectedThread()
  if (!thread?.cwd && !allowDetachedRoot) throw new Error(t('当前会话没有项目目录，无法安全打开文件。'))
  const root = String(file.root || thread?.cwd || '')
  if (!root) throw new Error(t('当前会话没有项目目录，无法安全打开文件。'))
  const path = fuzzyFileLabel(file)
  const requestedEpubCfi = String(file.epubCfi || '')
  if (!path) throw new Error(t('文件路径为空。'))
  if (requestedEpubCfi && state.artifact?.kind === 'epub' && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    if (epubReader) await epubReader.display(requestedEpubCfi)
    else {
      state.artifact.readingState = { ...(state.artifact.readingState || {}), cfi: requestedEpubCfi }
      renderArtifact()
    }
    return
  }
  if (state.artifact?.dirty && state.artifact.root === root && state.artifact.path === path) {
    activateRightWorkspace('document')
    return
  }
  if (state.artifact?.dirty && (state.artifact.root !== root || state.artifact.path !== path)) {
    if (!confirm(t('当前文档有尚未保存的修改，仍要打开其他文件吗？'))) return
  }
  const kind = previewableFileKind(file)
  if (!kind) throw new Error(t('此文件类型不能在文档审阅器中打开'))
  resetArtifactSearch()
  activateRightWorkspace('document')
  hideComposerMenu()
  closeActionMenus()
  $('#artifact-content').classList.add('hidden')
  $('#artifact-search-toolbar').classList.add('hidden')
  $('#artifact-error').classList.add('hidden')
  $('#artifact-loading').classList.remove('hidden')
  disposeArtifactEditor()
  disposeEpubReader()
  disposeRichArtifactReader()
  const requestId = randomId()
  state.artifact = { root, path, kind, requestId, threadKey: selectedStateKey(), returnTool, loading: true }
  const endpoint = kind === 'image'
    ? '/studio/review-image'
    : kind === 'epub' ? '/studio/review-epub'
      : kind === 'pdf' ? '/studio/review-pdf'
        : kind === 'table' && /\.xlsx$/iu.test(path) ? '/studio/review-spreadsheet'
          : '/studio/review-file'
  const response = await gatewayFetch(endpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, path }),
  })
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    const message = result?.error?.message || `HTTP ${response.status}`
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = { ...state.artifact, loading: false, error: message }
    renderArtifact()
    throw new Error(message)
  }
  if (kind === 'image') {
    const blob = await response.blob()
    if (!blob.type.startsWith('image/')) {
      const message = t('图片响应格式无效')
      if (state.artifact?.requestId === requestId) {
        state.artifact = { ...state.artifact, loading: false, error: message }
        renderArtifact()
      }
      throw new Error(message)
    }
    const imageUrl = await blobToDataUrl(blob)
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) {
      return
    }
    state.artifact = {
      ...state.artifact,
      loading: false,
      mimeType: blob.type,
      imageUrl,
      relativePath: path,
      size: blob.size,
    }
    state.artifactView = 'image'
  } else if (kind === 'epub') {
    const bookHash = response.headers.get('x-studio-epub-hash') || ''
    const bytes = await response.arrayBuffer()
    if (!bookHash || !bytes.byteLength) {
      const message = t('EPUB 响应格式无效')
      if (state.artifact?.requestId === requestId) {
        state.artifact = { ...state.artifact, loading: false, error: message }
        renderArtifact()
      }
      throw new Error(message)
    }
    const readingState = await loadEpubReadingState(root, path, bookHash)
    if (requestedEpubCfi) readingState.cfi = requestedEpubCfi
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = {
      ...state.artifact,
      kind: 'epub',
      loading: false,
      bytes,
      hash: bookHash,
      relativePath: path,
      size: bytes.byteLength,
      readingState,
    }
    state.artifactView = 'epub'
  } else if (kind === 'pdf' || (kind === 'table' && /\.xlsx$/iu.test(path))) {
    const bytes = await response.arrayBuffer()
    if (!bytes.byteLength) throw new Error(t('文档响应格式无效'))
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = {
      ...state.artifact, kind, loading: false, bytes,
      hash: response.headers.get('x-studio-content-hash') || '',
      relativePath: path, size: bytes.byteLength,
    }
    state.artifactView = kind
  } else {
    const result = await response.json()
    if (state.artifact?.requestId !== requestId || state.artifact.threadKey !== selectedStateKey()) return
    state.artifact = { ...result, kind: kind === 'table' ? 'table' : 'text', requestId, threadKey: selectedStateKey(), returnTool, loading: false }
    state.artifactView = kind === 'table' ? 'table' : isMarkdownFile(result.path) || isHtmlFile(result.path) ? 'preview' : 'source'
  }
  renderArtifact()
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
  hideSelectionPopover()
  syncRightWorkspaceLaunchers()
}

function syncRightWorkspaceLaunchers() {
  $('#open-thread-comments')?.setAttribute('aria-pressed', String(!$('#annotation-rail').classList.contains('hidden')))
  $('#open-thread-favorites')?.setAttribute('aria-pressed', String(!$('#favorites-rail').classList.contains('hidden')))
}

async function refreshArtifact() {
  if (!state.artifact) return
  if (state.artifact.dirty && !confirm(t('重新载入会丢失尚未保存的修改，是否继续？'))) return
  await openArtifact({ root: state.artifact.root, path: state.artifact.path }, { returnTool: state.artifact.returnTool })
}

function closeArtifactRail({ restoreMap = true, restoreWorkspace = true } = {}) {
  if (state.artifact?.dirty && !confirm(t('当前文档有尚未保存的修改，是否关闭？'))) return
  const returnTool = restoreWorkspace ? state.artifact?.returnTool : ''
  const artifactThreadKey = state.artifact?.threadKey
  $('#artifact-rail').classList.add('hidden')
  disposeArtifactEditor()
  disposeEpubReader()
  disposeRichArtifactReader()
  state.artifact = null
  resetArtifactSearch()
  hideSelectionPopover()
  if (returnTool && artifactThreadKey === selectedStateKey()) {
    workspaceTools.open(returnTool).catch(showError)
    return
  }
  if (restoreMap && $('#annotation-rail').classList.contains('hidden') && $('#favorites-rail').classList.contains('hidden')) renderSessionMap()
}

function setArtifactView(view) {
  if (!state.artifact || state.artifact.kind !== 'text') return
  if (view === 'preview' && !isMarkdownFile(state.artifact.path) && !isHtmlFile(state.artifact.path)) return
  if (!['preview', 'source', 'edit'].includes(view)) return
  state.artifactView = view
  renderArtifact()
}

async function saveArtifact({ overwrite = false } = {}) {
  const file = state.artifact
  if (!file || state.artifactView !== 'edit' || !file.dirty) return
  const content = artifactEditor?.value() ?? file.editContent ?? file.content
  file.saving = true
  $('#artifact-save').disabled = true
  const response = await gatewayFetch('/studio/workspace/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root: file.root, path: file.path, content, expectedHash: file.hash, overwrite }),
  })
  const result = await response.json().catch(() => null)
  if (response.status === 409 && result?.error?.code === 'workspace_file_conflict') {
    file.saving = false
    $('#artifact-save').disabled = false
    if (confirm(t('文件已在磁盘上发生变化。是否用当前编辑内容覆盖磁盘版本？'))) {
      await saveArtifact({ overwrite: true })
    }
    return
  }
  if (!response.ok) {
    file.saving = false
    $('#artifact-save').disabled = false
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
  state.artifact = {
    ...result,
    kind: 'text',
    requestId: file.requestId,
    threadKey: file.threadKey,
    returnTool: file.returnTool,
    loading: false,
    dirty: false,
    editContent: result.content,
  }
  renderArtifact()
  toast('文件已保存')
}

function renderArtifact() {
  const rail = $('#artifact-rail')
  const file = state.artifact
  if (!file) {
    disposeArtifactEditor()
    rail.classList.add('hidden')
    resetArtifactSearch()
    return
  }
  disposeArtifactEditor()
  disposeEpubReader()
  applyRightRailWidth()
  rail.classList.remove('hidden')
  $('#artifact-title').textContent = fileDisplayName(file.path)
  $('#artifact-path').textContent = file.relativePath || file.path
  const closeButton = $('#close-artifact')
  const returnLabel = file.returnTool === 'files' ? t('返回文件') : file.returnTool === 'review' ? t('返回 Git Review') : t('关闭文档')
  closeButton.classList.toggle('returning', Boolean(file.returnTool))
  closeButton.title = returnLabel
  closeButton.setAttribute('aria-label', returnLabel)
  $('#artifact-loading').classList.toggle('hidden', !file.loading)
  $('#artifact-error').classList.toggle('hidden', !file.error)
  $('#artifact-error-message').textContent = file.error || ''
  const textReady = !file.loading && !file.error && file.kind === 'text' && typeof file.content === 'string'
  const imageReady = !file.loading && !file.error && file.kind === 'image' && Boolean(file.imageUrl)
  const epubReady = !file.loading && !file.error && file.kind === 'epub' && file.bytes instanceof ArrayBuffer
  const pdfReady = !file.loading && !file.error && file.kind === 'pdf' && file.bytes instanceof ArrayBuffer
  const tableReady = !file.loading && !file.error && file.kind === 'table' && (file.bytes instanceof ArrayBuffer || typeof file.content === 'string')
  const ready = textReady || imageReady || epubReady || pdfReady || tableReady
  const content = $('#artifact-content')
  content.classList.toggle('hidden', !ready)
  $('#artifact-meta').textContent = textReady
    ? t('{lines} 行 · {size}', { lines: file.lineCount, size: formatFileSize(file.size) })
    : imageReady ? `${file.mimeType.replace('image/', '').toUpperCase()} · ${formatFileSize(file.size)}`
      : epubReady ? `EPUB · ${formatFileSize(file.size)}`
        : pdfReady ? `PDF · ${formatFileSize(file.size)}`
          : tableReady ? `${/\.xlsx$/iu.test(file.path) ? 'XLSX' : 'CSV'} · ${formatFileSize(file.size)}` : ''
  $('#artifact-hint').textContent = t(file.kind === 'image' ? '图片预览不支持批注' : file.kind === 'epub' ? '选择书中文字，添加问题后交给 AI' : file.kind === 'pdf' ? '选择 PDF 文字，或按住 Shift 拖拽区域即可批注' : file.kind === 'table' ? '选择单元格即可批注' : '选择文字即可批注')
  const markdown = textReady && isMarkdownFile(file.path)
  const html = textReady && isHtmlFile(file.path)
  const editable = textReady
  const renderedContent = file.editContent ?? file.content
  $('#artifact-title').textContent = `${fileDisplayName(file.path)}${file.dirty ? ' •' : ''}`
  $('#artifact-view-switch').classList.toggle('hidden', !editable)
  $('#artifact-preview').disabled = !markdown && !html
  $('#artifact-preview').classList.toggle('active', state.artifactView === 'preview')
  $('#artifact-source').classList.toggle('active', state.artifactView === 'source')
  $('#artifact-edit').classList.toggle('active', state.artifactView === 'edit')
  $('#artifact-save').classList.toggle('hidden', state.artifactView !== 'edit')
  $('#artifact-save').disabled = !file.dirty || Boolean(file.saving)
  const canSearch = artifactSearchAvailable(file, state.artifactView)
  $('#artifact-search-toolbar').classList.toggle('hidden', !canSearch)
  $('#artifact-search-input').setAttribute('placeholder', t('搜索文档内容…'))
  $('#artifact-search-prev').title = t('上一个匹配')
  $('#artifact-search-prev').setAttribute('aria-label', t('上一个匹配'))
  $('#artifact-search-next').title = t('下一个匹配')
  $('#artifact-search-next').setAttribute('aria-label', t('下一个匹配'))
  if (!ready) return
  if (imageReady) {
    content.className = 'artifact-content artifact-image-preview'
    content.innerHTML = `<div class="artifact-image-stage"><img src="${escapeHtml(file.imageUrl)}" alt="${escapeHtml(fileDisplayName(file.path))}" draggable="false" /></div>`
    const image = content.querySelector('img')
    image?.addEventListener('load', () => {
      if (state.artifact?.requestId !== file.requestId) return
      $('#artifact-meta').textContent = `${image.naturalWidth} × ${image.naturalHeight} · ${file.mimeType.replace('image/', '').toUpperCase()} · ${formatFileSize(file.size)}`
    }, { once: true })
    image?.addEventListener('error', () => {
      if (state.artifact?.requestId !== file.requestId) return
      state.artifact = { ...state.artifact, error: t('无法解码图片') }
      renderArtifact()
    }, { once: true })
    renderArtifactSearchStatus()
    return
  }
  if (epubReady) {
    content.className = 'artifact-content artifact-epub-preview'
    content.innerHTML = '<div class="artifact-epub-host" data-no-i18n></div>'
    mountEpubReader(file, content.firstElementChild).catch((error) => {
      if (state.artifact?.requestId !== file.requestId) return
      state.artifact = { ...state.artifact, error: error.message || String(error) }
      renderArtifact()
    })
    renderArtifactSearchStatus()
    return
  }
  if (pdfReady) {
    content.className = 'artifact-content artifact-pdf-preview'
    content.innerHTML = '<div class="artifact-pdf-host" data-no-i18n></div>'
    mountPdfReader(file, content.firstElementChild).catch(showError)
    renderArtifactSearchStatus()
    return
  }
  if (tableReady) {
    content.className = 'artifact-content artifact-table-preview'
    content.innerHTML = '<div class="artifact-table-host" data-no-i18n></div>'
    mountTableReader(file, content.firstElementChild).catch(showError)
    renderArtifactSearchStatus()
    return
  }
  if (state.artifactView === 'edit') {
    content.className = 'artifact-content editing'
    content.innerHTML = '<div class="artifact-editor-shell" data-no-i18n></div>'
    mountArtifactEditor(file, content.firstElementChild, renderedContent).catch(showError)
  } else if (markdown && state.artifactView === 'preview') {
    content.className = 'artifact-content markdown-body'
    content.innerHTML = renderMarkdown(renderedContent)
  } else if (html && state.artifactView === 'preview') {
    content.className = 'artifact-content markdown-body artifact-html-preview'
    content.innerHTML = renderStaticHtml(renderedContent)
  } else {
    content.className = 'artifact-content'
    content.innerHTML = `<pre class="artifact-source" data-no-i18n>${escapeHtml(renderedContent)}</pre>`
  }
  if (state.artifactSearch) applyArtifactSearchHighlights()
  else renderArtifactSearchStatus()
}

function disposeArtifactEditor() {
  artifactEditor?.destroy()
  artifactEditor = null
}

function disposeEpubReader() {
  epubReaderGeneration += 1
  clearTimeout(epubReadingStateTimer)
  epubReadingStateTimer = null
  if (state.artifact?.kind === 'epub' && state.artifact.readingState) {
    persistEpubReadingState(state.artifact).catch(() => {})
  }
  epubReader?.destroy()
  epubReader = null
}

function disposeRichArtifactReader() {
  richArtifactReader?.destroy?.()
  richArtifactReader = null
}

async function mountPdfReader(file, parent) {
  pdfReaderModule ||= import('./pdf-reader.mjs')
  const { createPdfReader } = await pdfReaderModule
  if (state.artifact !== file || !parent.isConnected) return
  richArtifactReader = await createPdfReader({
    container: parent, bytes: file.bytes, initialPage: file.page || 1, search: state.artifactSearch,
    translate: t,
    onPageChange: (page) => { file.page = page },
    onSelection: (selection) => {
      if (state.artifact !== file || !selection.quote) return
      state.pendingSelection = { quote: selection.quote, itemId: null, turnId: null, source: pdfCommentSource({ root: file.root, filePath: file.path, documentHash: file.hash, page: selection.page, rects: selection.rects }) }
      positionSelectionPopover(selection.rect, { allowFavorite: false })
    },
  })
}

async function mountTableReader(file, parent) {
  tableReaderModule ||= import('./table-reader.mjs')
  const { parseTabularArtifact, renderTableArtifact } = await tableReaderModule
  if (!file.workbook) file.workbook = await parseTabularArtifact({ bytes: file.bytes, path: file.path, text: file.content })
  if (state.artifact !== file || !parent.isConnected) return
  richArtifactReader = renderTableArtifact({
    container: parent, workbook: file.workbook, translate: t,
    onSelection: (selection) => {
      if (state.artifact !== file) return
      state.pendingSelection = { quote: selection.quote || `[${selection.range}]`, itemId: null, turnId: null, source: tableCommentSource({ root: file.root, filePath: file.path, documentHash: file.hash, sheet: selection.sheet, range: selection.range }) }
      positionSelectionPopover(selection.rect, { allowFavorite: false })
    },
  })
}

async function loadEpubReadingState(root, path, bookHash) {
  const response = await gatewayFetch('/studio/epub/state', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, path, bookHash }),
  })
  if (response.status === 204) return {}
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
  return response.json()
}

async function mountEpubReader(file, parent) {
  const generation = ++epubReaderGeneration
  epubReaderModule ||= import('./epub-reader.mjs')
  const { createEpubReader } = await epubReaderModule
  if (generation !== epubReaderGeneration || state.artifact !== file || !parent.isConnected) return
  const reader = await createEpubReader({
    container: parent,
    bytes: file.bytes,
    initialState: file.readingState,
    translate: t,
    onSelection: (selection) => captureEpubSelection(file, selection),
    onRelocate: (readingState) => {
      if (state.artifact !== file) return
      file.readingState = readingState
      scheduleEpubReadingState(file)
    },
    onExternalLink: (url) => openBrowserUrl(url).catch(showError),
  })
  if (generation !== epubReaderGeneration || state.artifact !== file || !parent.isConnected) {
    reader.destroy()
    return
  }
  epubReader = reader
  file.bookTitle = reader.title
  if (file.bookTitle) $('#artifact-title').textContent = file.bookTitle
}

function scheduleEpubReadingState(file) {
  clearTimeout(epubReadingStateTimer)
  epubReadingStateTimer = setTimeout(() => {
    epubReadingStateTimer = null
    persistEpubReadingState(file).catch((error) => reportClientError(new Error(`EPUB reading state: ${error?.message || error}`)))
  }, 450)
}

async function persistEpubReadingState(file) {
  if (!file?.readingState || !file.hash) return
  const reading = file.readingState
  const response = await gatewayFetch('/studio/epub/state', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      root: file.root,
      path: file.path,
      bookHash: file.hash,
      cfi: reading.cfi || '',
      chapterLabel: reading.chapterLabel || '',
      progress: Number(reading.progress) || 0,
      fontScale: Number(reading.fontScale) || 1,
      theme: reading.theme || 'light',
      flow: reading.flow || 'paginated',
      tocOpen: Boolean(reading.tocOpen),
    }),
  })
  if (!response.ok) {
    const result = await response.json().catch(() => null)
    throw new Error(result?.error?.message || `HTTP ${response.status}`)
  }
}

function captureEpubSelection(file, selection) {
  if (state.artifact !== file || !selection?.quote) return
  state.pendingSelection = {
    quote: selection.quote,
    itemId: null,
    turnId: null,
    source: epubCommentSource({
      root: file.root,
      filePath: file.path,
      bookHash: file.hash,
      cfiRange: selection.cfiRange,
      href: selection.href,
      chapterLabel: selection.chapterLabel,
    }),
  }
  positionSelectionPopover(selection.rect, { allowFavorite: false })
}

async function reopenEpubComment(anchor) {
  await openArtifact({
    root: anchor.root || selectedThread()?.cwd,
    path: anchor.filePath,
    epubCfi: anchor.cfiRange,
  })
}

async function mountArtifactEditor(file, parent, content) {
  workspaceEditorModule ||= import('./workspace-editor.mjs')
  const { createWorkspaceEditor } = await workspaceEditorModule
  if (state.artifact !== file || state.artifactView !== 'edit' || !parent.isConnected) return
  artifactEditor = createWorkspaceEditor({
    parent,
    content,
    language: file.language,
    onSave: () => saveArtifact().catch(showError),
    onChange: (value) => {
      file.editContent = value
      file.dirty = file.editContent !== file.content
      const lines = file.editContent ? file.editContent.split('\n').length : 1
      $('#artifact-title').textContent = `${fileDisplayName(file.path)}${file.dirty ? ' •' : ''}`
      $('#artifact-save').disabled = !file.dirty
      $('#artifact-meta').textContent = t('{lines} 行 · {size}', { lines, size: formatFileSize(new TextEncoder().encode(file.editContent).length) })
    },
  })
  artifactEditor.focus()
}

function renderStaticHtml(value) {
  const clean = DOMPurify.sanitize(String(value || ''), {
    USE_PROFILES: { html: true },
    FORBID_TAGS: STATIC_HTML_FORBIDDEN_TAGS,
    FORBID_ATTR: STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  })
  const template = document.createElement('template')
  template.innerHTML = clean
  template.content.querySelectorAll('*').forEach((element) => {
    for (const attribute of [...element.attributes]) {
      if (/^on/iu.test(attribute.name)) element.removeAttribute(attribute.name)
    }
  })
  return template.innerHTML
}

function blobToDataUrl(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.addEventListener('load', () => resolve(String(reader.result || '')), { once: true })
    reader.addEventListener('error', () => reject(reader.error || new Error(t('无法解码图片'))), { once: true })
    reader.readAsDataURL(blob)
  })
}

function resetArtifactSearch() {
  state.artifactSearch = ''
  state.artifactSearchIndex = -1
  state.artifactSearchMatches = []
  const searchInput = $('#artifact-search-input')
  if (searchInput) searchInput.value = ''
  clearArtifactSearchHighlights()
  renderArtifactSearchStatus()
  clearTimeout(artifactSearchTimer)
}

function handleArtifactSearchInput(event) {
  state.artifactSearch = event.target.value
  clearTimeout(artifactSearchTimer)
  if (!state.artifact || !state.artifactSearch.trim()) {
    clearArtifactSearchHighlights()
    renderArtifactSearchStatus()
    return
  }
  artifactSearchTimer = setTimeout(() => {
    const content = $('#artifact-content')
    if (!content || content.classList.contains('hidden')) return
    applyArtifactSearchHighlights()
  }, 140)
}

function handleArtifactSearchKeydown(event) {
  if (event.key !== 'Enter') return
  event.preventDefault()
  if (!state.artifactSearch.trim() || state.artifactSearchMatches.length === 0) return
  navigateArtifactSearch(event.shiftKey ? -1 : 1)
}

function handleArtifactSearchMatches(raw = '') {
  const content = $('#artifact-content')
  if (!content || !raw.trim()) return []
  const query = raw.trim()
  const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue) return NodeFilter.FILTER_SKIP
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (parent.closest('button, .markdown-code-header, .markdown-mermaid-canvas, script, style, textarea, .artifact-search-highlight')) {
        return NodeFilter.FILTER_REJECT
      }
      const mermaidSource = parent.closest('.markdown-mermaid-source')
      if (mermaidSource && !mermaidSource.closest('.markdown-mermaid.show-source')) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })

  const matches = []
  while (walker.nextNode()) {
    const node = walker.currentNode
    findTextMatchRanges(node.nodeValue, query).forEach(({ start, end }) => {
      matches.push({ node, start, end })
    })
  }
  return matches
}

function clearArtifactSearchHighlights() {
  const content = $('#artifact-content')
  if (!content) return
  content.querySelectorAll('mark.artifact-search-highlight').forEach((mark) => {
    const parent = mark.parentElement
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  })
  state.artifactSearchMatches = []
  state.artifactSearchIndex = -1
}

function applyArtifactSearchHighlights() {
  const content = $('#artifact-content')
  if (!content || !artifactSearchAvailable(state.artifact, state.artifactView)) {
    clearArtifactSearchHighlights()
    return
  }
  clearArtifactSearchHighlights()
  const query = state.artifactSearch.trim()
  if (!query) {
    renderArtifactSearchStatus()
    return
  }

  const nodeGroups = new Map()
  handleArtifactSearchMatches(query).forEach(({ node, start, end }) => {
    if (!nodeGroups.has(node)) nodeGroups.set(node, [])
    nodeGroups.get(node).push([start, end])
  })
  nodeGroups.forEach((positions, node) => {
    const text = node.nodeValue
    const parent = node.parentNode
    if (!parent) return
    const fragment = document.createDocumentFragment()
    let cursor = 0
    positions.sort((a, b) => a[0] - b[0]).forEach(([start, end]) => {
      if (start < cursor || end > text.length) return
      if (start > cursor) fragment.appendChild(document.createTextNode(text.slice(cursor, start)))
      const mark = document.createElement('mark')
      mark.className = 'artifact-search-highlight'
      mark.textContent = text.slice(start, end)
      fragment.appendChild(mark)
      cursor = end
    })
    if (cursor < text.length) fragment.appendChild(document.createTextNode(text.slice(cursor)))
    parent.replaceChild(fragment, node)
  })

  state.artifactSearchMatches = [...content.querySelectorAll('mark.artifact-search-highlight')]
  state.artifactSearchIndex = state.artifactSearchMatches.length ? 0 : -1
  if (state.artifactSearchMatches.length) scrollArtifactSearchToMatch(0, { behavior: 'auto' })
  renderArtifactSearchStatus()
}

function renderArtifactSearchStatus() {
  const summary = $('#artifact-search-summary')
  const total = state.artifactSearchMatches.length
  const query = state.artifactSearch.trim()
  if (!summary) return
  if (!query || !artifactSearchAvailable(state.artifact, state.artifactView)) {
    summary.textContent = ''
    $('#artifact-search-prev').disabled = true
    $('#artifact-search-next').disabled = true
    return
  }
  summary.textContent = total
    ? t('{current} / {total}', { current: state.artifactSearchIndex + 1, total })
    : t('未找到匹配内容')
  $('#artifact-search-prev').disabled = total === 0
  $('#artifact-search-next').disabled = total === 0
}

function scrollArtifactSearchToMatch(index, { behavior = 'smooth' } = {}) {
  if (!state.artifactSearchMatches.length) return
  const current = ((index % state.artifactSearchMatches.length) + state.artifactSearchMatches.length) % state.artifactSearchMatches.length
  state.artifactSearchIndex = current
  state.artifactSearchMatches.forEach((match, matchIndex) => {
    match.classList.toggle('current', matchIndex === current)
  })
  const target = state.artifactSearchMatches[current]
  if (target?.isConnected) target.scrollIntoView({ behavior, block: 'center', inline: 'nearest' })
  renderArtifactSearchStatus()
}

function navigateArtifactSearch(step) {
  if (!state.artifactSearchMatches.length) return
  const next = state.artifactSearchIndex < 0 ? 0 : state.artifactSearchIndex + step
  scrollArtifactSearchToMatch(next)
}

function formatFileSize(value) {
  const bytes = Number(value) || 0
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function captureTranscriptSelection() {
  const selection = window.getSelection()
  const text = selection?.toString().trim()
  if (!text || selection.rangeCount === 0) return hideSelectionPopover()
  const range = selection.getRangeAt(0)
  const transcript = $('#transcript')
  if (!transcript.contains(range.commonAncestorContainer)) return hideSelectionPopover()
  const element = range.commonAncestorContainer.nodeType === Node.ELEMENT_NODE
    ? range.commonAncestorContainer
    : range.commonAncestorContainer.parentElement
  const item = element?.closest('[data-item-id]')
  const turn = element?.closest('[data-turn-id]')
  state.pendingSelection = {
    quote: text.slice(0, 16000),
    itemId: item?.dataset.itemId || null,
    turnId: item?.dataset.turnId || turn?.dataset.turnId || null,
    source: chatCommentSource({
      itemId: item?.dataset.itemId || null,
      turnId: item?.dataset.turnId || turn?.dataset.turnId || null,
    }),
  }
  positionSelectionPopover(range, { allowFavorite: true })
}

function captureArtifactSelection() {
  const selection = window.getSelection()
  const text = selection?.toString().trim()
  if (state.artifact?.kind === 'image') return hideSelectionPopover()
  if (!state.artifact || !text || selection.rangeCount === 0) return hideSelectionPopover()
  const range = selection.getRangeAt(0)
  const content = $('#artifact-content')
  if (!content.contains(range.commonAncestorContainer)) return hideSelectionPopover()
  let hintOffset = 0
  if (state.artifactView === 'source') {
    const source = content.querySelector('.artifact-source')
    if (source) {
      const prefix = document.createRange()
      prefix.selectNodeContents(source)
      prefix.setEnd(range.startContainer, range.startOffset)
      hintOffset = prefix.toString().length
    }
  }
  state.pendingSelection = {
    quote: text.slice(0, 16000),
    itemId: null,
    turnId: null,
    source: documentCommentSource(createFileRangeTarget(state.artifact, text, hintOffset)),
  }
  positionSelectionPopover(range, { allowFavorite: false })
}

function positionSelectionPopover(range, { allowFavorite }) {
  const rect = typeof range?.getBoundingClientRect === 'function' ? range.getBoundingClientRect() : range
  if (!rect) return hideSelectionPopover()
  const popover = $('#selection-popover')
  popover.style.left = `${Math.min(window.innerWidth - 150, Math.max(8, rect.left + rect.width / 2 - 55))}px`
  popover.style.top = `${Math.max(8, rect.top - 39)}px`
  $('#selection-favorite').classList.toggle('hidden', !allowFavorite)
  popover.classList.remove('hidden')
}

function openAnnotationFromSelection() {
  if (!state.pendingSelection?.quote) {
    captureTranscriptSelection()
    if (!state.pendingSelection?.quote) return toast('请先在 Codex 输出中选择文字', 'error')
  }
  state.pendingAnnotation = commentSelectionSnapshot(state.pendingSelection, commentSources)
  if (!state.pendingAnnotation) return toast('请重新选择需要批注的文字', 'error')
  $('#annotation-quote').textContent = state.pendingAnnotation.excerpt
  $('#annotation-source-hint').textContent = commentSources.describe(state.pendingAnnotation, commentProviderContext(0))
  $('#annotation-comment').value = ''
  $('#annotation-comment').placeholder = state.pendingAnnotation.source?.provider === 'epub'
    ? t('例如：解释这段内容的核心含义、上下文和关键概念。')
    : t('说明问题和期望调整，也可以直接将选中内容加入草稿。')
  $('#annotation-error').classList.add('hidden')
  hideSelectionPopover(false)
  $('#annotation-dialog').showModal()
  setTimeout(() => $('#annotation-comment').focus(), 30)
}

function openFavoriteFromSelection() {
  if (!state.pendingSelection?.quote) {
    captureTranscriptSelection()
    if (!state.pendingSelection?.quote) return toast('请先在 AI 输出中选择文字', 'error')
  }
  const thread = selectedThread()
  const turn = state.model.turns.find((candidate) => String(candidate.id) === String(state.pendingSelection.turnId))
  if (!thread || !state.pendingSelection.turnId || !state.pendingSelection.itemId) {
    return toast('无法确定所选文字的消息位置，请在一条回复内选择', 'error')
  }
  state.favoriteEditMode = false
  state.pendingFavorite = {
    id: randomId(),
    scope: 'selection',
    backend: state.backend,
    threadId: state.selectedId,
    threadTitle: threadTitle(thread),
    projectPath: thread.cwd || '',
    turnId: String(state.pendingSelection.turnId),
    itemId: String(state.pendingSelection.itemId),
    title: autoFavoriteTitle(state.pendingSelection.quote),
    question: turn ? questionForTurn(turn) : '',
    content: state.pendingSelection.quote,
    note: '',
    tags: [],
    createdAt: new Date().toISOString(),
  }
  hideSelectionPopover(false)
  populateFavoriteDialog(state.pendingFavorite)
}

function hideSelectionPopover(clear = true) {
  $('#selection-popover').classList.add('hidden')
  if (clear) state.pendingSelection = null
}

function closeAnnotationDialog() {
  $('#annotation-dialog').close()
  state.pendingAnnotation = null
  state.pendingSelection = null
  window.getSelection()?.removeAllRanges()
}

function currentAnnotations() {
  return state.selectedId ? state.annotationDrafts[selectedStateKey()] || [] : []
}

function addAnnotation(event) {
  event.preventDefault()
  const comment = $('#annotation-comment').value.trim()
  const errorBox = $('#annotation-error')
  const annotation = state.pendingAnnotation
  if (!state.selectedId || !annotation?.excerpt) {
    errorBox.textContent = '选中内容不能为空。'
    errorBox.classList.remove('hidden')
    return
  }
  const drafts = currentAnnotations()
  if (drafts.length >= 32) {
    errorBox.textContent = '每个会话最多保留 32 条批注。'
    errorBox.classList.remove('hidden')
    return
  }
  const draft = createCommentDraft({ ...annotation, note: comment }, { registry: commentSources })
  state.annotationDrafts[selectedStateKey()] = [...drafts, draft]
  persistPreferences()
  closeAnnotationDialog()
  renderAnnotationRail()
  renderComposerReviewContext()
  toast('批注已加入回复草稿')
}

function openAnnotationRail() {
  activateRightWorkspace('comments')
  renderAnnotationRail()
}
function closeAnnotationRail() {
  $('#annotation-rail').classList.add('hidden')
  if (state.activeRightWorkspace === 'comments') state.activeRightWorkspace = null
  syncRightWorkspaceLaunchers()
  if ($('#favorites-rail').classList.contains('hidden')) {
    if (state.artifact) renderArtifact()
    else renderSessionMap()
  }
}

function renderAnnotationRail() {
  const drafts = currentAnnotations()
  $('#annotation-count').textContent = drafts.length
  const commentsButton = $('#open-thread-comments')
  const commentsLabel = drafts.length ? `${t('批注')} · ${drafts.length}` : t('批注')
  commentsButton.title = commentsLabel
  commentsButton.setAttribute('aria-label', commentsLabel)
  $('#annotation-empty').classList.toggle('hidden', drafts.length > 0)
  $('#annotation-list').classList.toggle('hidden', drafts.length === 0)
  $('#clear-annotations').disabled = !drafts.length && !state.annotationAdditional[selectedStateKey()]
  $('#insert-annotations').disabled = !drafts.length
  $('#annotation-additional').value = state.selectedId ? state.annotationAdditional[selectedStateKey()] || '' : ''
  $('#annotation-list').innerHTML = drafts.map((draft, index) => `<article class="annotation-card" data-draft-id="${escapeHtml(draft.id)}">
    <header><button class="annotation-source" type="button">${escapeHtml(annotationSourceLabel(draft, index))}</button><button class="annotation-delete" type="button" aria-label="${t('删除批注 {index}', { index: index + 1 })}">×</button></header>
    <blockquote>${escapeHtml(draft.excerpt)}</blockquote>${draft.note ? `<p>${escapeHtml(draft.note)}</p>` : ''}
  </article>`).join('')
  $$('.annotation-delete').forEach((button) => button.addEventListener('click', () => deleteAnnotation(button.closest('.annotation-card').dataset.draftId)))
  $$('.annotation-source').forEach((button) => button.addEventListener('click', () => reopenAnnotationSource(button.closest('.annotation-card').dataset.draftId).catch(showError)))
  renderComposerReviewContext()
}

function annotationSourceLabel(draft, index) {
  return commentSources.describe(draft, commentProviderContext(index))
}

async function reopenAnnotationSource(id) {
  const draft = currentAnnotations().find((candidate) => candidate.id === id)
  if (!draft) return
  await commentSources.reopen(draft, commentProviderContext())
}

async function reopenDocumentComment(target, excerpt) {
  await openArtifact({ root: target.root || selectedThread()?.cwd, path: target.filePath })
  setArtifactView('source')
  const source = $('#artifact-content .artifact-source')
  if (!source) return
  const { startOffset, endOffset } = relocateDocumentComment({ anchor: target }, state.artifact, excerpt)
  if (startOffset == null || endOffset == null || endOffset <= startOffset) return
  const node = source.firstChild
  if (!node) return
  const range = document.createRange()
  range.setStart(node, Math.min(startOffset, node.length))
  range.setEnd(node, Math.min(endOffset, node.length))
  const selection = window.getSelection()
  selection.removeAllRanges()
  selection.addRange(range)
  const rect = range.getBoundingClientRect()
  $('#artifact-content').scrollBy({ top: rect.top - $('#artifact-content').getBoundingClientRect().top - 90, behavior: 'smooth' })
}

function renderComposerReviewContext() {
  const drafts = currentAnnotations()
  const context = $('#composer-review-context')
  context.classList.toggle('hidden', !drafts.length)
  if (!drafts.length) return
  $('#composer-review-count').textContent = t('{count} 条批注待发送', { count: drafts.length })
  $('#composer-review-source').textContent = t('等待加入消息')
}

function deleteAnnotation(id) {
  if (!state.selectedId) return
  const key = selectedStateKey()
  state.annotationDrafts[key] = currentAnnotations().filter((draft) => draft.id !== id)
  if (!state.annotationDrafts[key].length) delete state.annotationDrafts[key]
  persistPreferences()
  renderAnnotationRail()
}

function clearAnnotations() {
  if (!state.selectedId || !confirm(t('清空当前会话的全部批注草稿？'))) return
  delete state.annotationDrafts[selectedStateKey()]
  delete state.annotationAdditional[selectedStateKey()]
  persistPreferences()
  renderAnnotationRail()
}

function saveAnnotationAdditional(event) {
  if (!state.selectedId) return
  const value = event.target.value.slice(0, 32000)
  if (value) state.annotationAdditional[selectedStateKey()] = value
  else delete state.annotationAdditional[selectedStateKey()]
  clearTimeout(annotationPersistTimer)
  annotationPersistTimer = setTimeout(persistPreferences, 300)
}

function buildAnnotationPrompt(drafts, additional = '') {
  const annotations = drafts.map((draft, index) => {
    const anchor = commentSources.promptAnchor(draft, commentProviderContext(index))
    const quote = draft.excerpt.split('\n').map((line) => `> ${line}`).join('\n')
    return t(anchor
      ? '批注 {index}（{anchor}）\n引用：\n{quote}\n\n我的意见：\n{comment}'
      : '批注 {index}\n引用：\n{quote}\n\n我的意见：\n{comment}', {
      index: index + 1,
      anchor,
      quote,
      comment: draft.note || t('无补充意见'),
    })
  }).join('\n\n---\n\n')
  const additionalBlock = additional.trim() ? t('整体补充：\n{text}', { text: additional.trim() }) : ''
  return [...commentSources.promptInstructions(drafts, commentProviderContext()), state.annotationPromptTemplate
    .replaceAll('{{annotations}}', annotations)
    .replaceAll('{{additional}}', additionalBlock)
    .replace(/\n{3,}/g, '\n\n')
    .trim()].filter(Boolean).join('\n\n')
}

function commentProviderContext(index = 0) {
  return {
    index,
    translate: t,
    unknownLabel: t('已保存批注'),
    contentForSource: (source) => state.artifact?.path === source?.anchor?.filePath ? state.artifact.content : null,
    openDocument: reopenDocumentComment,
    openEpubSource: reopenEpubComment,
    openPdfSource: reopenPdfComment,
    openTableSource: reopenTableComment,
    openWebSource: openBrowserUrl,
  }
}

async function reopenPdfComment(anchor) {
  await openArtifact({ root: anchor.root || selectedThread()?.cwd, path: anchor.filePath })
  state.artifact.page = anchor.page
  await richArtifactReader?.goToPage?.(anchor.page)
}

async function reopenTableComment(anchor) {
  await openArtifact({ root: anchor.root || selectedThread()?.cwd, path: anchor.filePath })
}

function insertAnnotations() {
  const drafts = currentAnnotations()
  if (!drafts.length) return
  const prompt = buildAnnotationPrompt(drafts, state.annotationAdditional[selectedStateKey()] || '')
  const composer = $('#composer-input')
  composer.value = [composer.value.trim(), prompt].filter(Boolean).join('\n\n')
  closeAnnotationRail()
  composer.focus()
  renderComposerReviewContext()
  toast('批注草稿已插入输入框')
}

async function favoriteRequest(path, options = {}) {
  const response = await gatewayFetch(path, {
    cache: 'no-store',
    ...options,
    headers: options.body ? { 'Content-Type': 'application/json', ...(options.headers || {}) } : options.headers,
  })
  const text = await response.text()
  let value = null
  try { value = text ? JSON.parse(text) : null } catch { value = text }
  if (!response.ok) throw new Error(value?.error?.message || value?.message || `HTTP ${response.status}`)
  return value
}

async function loadFavorites() {
  const query = encodeURIComponent(state.favoriteQuery)
  const displayLimit = state.favoriteQuery ? 300 : 2000
  const [result, indexResult] = await Promise.all([
    favoriteRequest(`/studio/favorites?q=${query}&limit=${displayLimit}`),
    state.favoriteQuery
      ? favoriteRequest('/studio/favorites?limit=2000')
      : Promise.resolve(null),
  ])
  state.favorites = Array.isArray(result?.items) ? result.items : []
  state.favoriteIndex = Array.isArray(indexResult?.items) ? indexResult.items : state.favorites
  state.favoriteTotal = Number(result?.allTotal || 0)
  renderFavoritesRail()
  syncFavoriteButtons()
}

function favoriteForSource(backend, threadId, turnId, itemId) {
  const key = favoriteSourceKey({ backend, threadId, turnId, itemId })
  return state.favoriteIndex.find((favorite) => favorite.scope !== 'selection' && favoriteSourceKey(favorite) === key) || null
}

function openFavoritesRail(scope = 'global') {
  state.favoriteScope = scope
  activateRightWorkspace('favorites')
  loadFavorites().catch(showError)
  setTimeout(() => $('#favorites-search').focus(), 30)
}

function closeFavoritesRail() {
  $('#favorites-rail').classList.add('hidden')
  if (state.activeRightWorkspace === 'favorites') state.activeRightWorkspace = null
  syncRightWorkspaceLaunchers()
  if ($('#annotation-rail').classList.contains('hidden')) {
    if (state.artifact) renderArtifact()
    else renderSessionMap()
  }
}

async function exportFavorites() {
  const response = await gatewayFetch('/studio/favorites/export', { cache: 'no-store' })
  if (!response.ok) throw new Error(await response.text() || `HTTP ${response.status}`)
  const url = URL.createObjectURL(await response.blob())
  const link = document.createElement('a')
  link.href = url
  link.download = 'codex-thread-studio-favorites.md'
  document.body.append(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  toast('收藏已导出')
}

function handleFavoritesSearch(event) {
  state.favoriteQuery = event.target.value.trim()
  clearTimeout(favoritesSearchTimer)
  favoritesSearchTimer = setTimeout(() => loadFavorites().catch(showError), 180)
}

function renderFavoritesRail() {
  const visibleFavorites = state.favoriteScope === 'session' && state.selectedId
    ? state.favorites.filter((favorite) => favorite.backend === state.backend && favorite.threadId === state.selectedId)
    : state.favorites
  const count = state.favoriteScope === 'session' ? visibleFavorites.length : state.favoriteTotal
  $('#favorites-title').textContent = t(state.favoriteScope === 'session' ? '本会话收藏' : '全局收藏')
  $('#export-favorites').classList.toggle('hidden', state.favoriteScope !== 'global')
  $('#favorites-count').textContent = count
  $('#favorites-badge').textContent = count > 99 ? '99+' : count
  $('#favorites-badge').classList.toggle('hidden', count === 0)
  $('#favorites-search-summary').textContent = state.favoriteQuery
    ? t('找到 {count} 条匹配收藏', { count: visibleFavorites.length })
    : state.favoriteScope === 'session'
      ? t('{count} 条当前会话收藏', { count })
      : t('{count} 条跨会话结构化收藏', { count })
  const empty = visibleFavorites.length === 0
  $('#favorites-empty').classList.toggle('hidden', !empty)
  $('#favorites-list').classList.toggle('hidden', empty)
  $('#favorites-empty strong').textContent = state.favoriteQuery ? '没有匹配结果' : '还没有收藏'
  $('#favorites-empty p').textContent = state.favoriteQuery
    ? '试试回复中的关键词、会话名称或标签。'
    : '将鼠标移到任意 AI 回复上，点击右上角的收藏按钮。'
  $('#favorites-list').innerHTML = visibleFavorites.map((favorite) => {
    const tags = favorite.tags?.length
      ? `<div class="favorite-card-tags">${favorite.tags.slice(0, 4).map((tag) => `<span>${escapeHtml(tag)}</span>`).join('')}</div>`
      : ''
    const question = favorite.questionSnippet
      ? `<p class="favorite-card-question"><span>Q</span>${escapeHtml(favorite.questionSnippet)}</p>`
      : ''
    return `<button class="favorite-card" type="button" data-favorite-id="${escapeHtml(favorite.id)}">
      <div class="favorite-card-top"><span class="favorite-backend-pill ${escapeHtml(favorite.backend)}">${escapeHtml(favorite.backend)}</span><time>${escapeHtml(formatFavoriteDate(favorite.createdAt))}</time></div>
      <strong>${escapeHtml(favorite.title)}</strong>
      ${question}
      <p class="favorite-card-answer">${escapeHtml(favorite.snippet)}</p>
      ${tags}
      <footer><span>${escapeHtml(favorite.threadTitle || t('未命名会话'))}</span><span>${escapeHtml(basename(favorite.projectPath))}</span></footer>
    </button>`
  }).join('')
  renderSessionFavoriteCount()
}

function renderSessionFavoriteCount() {
  const count = state.selectedId
    ? state.favoriteIndex.filter((favorite) => favorite.backend === state.backend && favorite.threadId === state.selectedId).length
    : 0
  const favoritesButton = $('#open-thread-favorites')
  const favoritesLabel = count ? `${t('收藏')} · ${count}` : t('收藏')
  favoritesButton.title = favoritesLabel
  favoritesButton.setAttribute('aria-label', favoritesLabel)
}

function handleFavoriteListClick(event) {
  const card = event.target.closest('[data-favorite-id]')
  if (card) openFavoriteDetail(card.dataset.favoriteId).catch(showError)
}

function openFavoriteForMessage(turnId, itemId) {
  const turn = state.model.turns.find((candidate) => String(candidate.id) === String(turnId))
  const item = turn?.items?.find((candidate) => String(candidate.id) === String(itemId))
  const thread = selectedThread()
  const visibleText = item?.type === 'agentMessage' ? sessionMapVisibleText(item.text) : item?.text || ''
  if (!turn || !item || !thread || !visibleText.trim()) {
    toast('这条回复尚未完成，暂时不能收藏', 'error')
    return
  }
  state.favoriteEditMode = false
  state.pendingFavorite = {
    id: randomId(),
    scope: 'message',
    backend: state.backend,
    threadId: state.selectedId,
    threadTitle: threadTitle(thread),
    projectPath: thread.cwd || '',
    turnId: String(turn.id || ''),
    itemId: String(item.id || ''),
    title: autoFavoriteTitle(visibleText),
    question: questionForTurn(turn),
    content: visibleText.trim(),
    note: '',
    tags: [],
    createdAt: new Date().toISOString(),
  }
  populateFavoriteDialog(state.pendingFavorite)
}

function populateFavoriteDialog(favorite) {
  const editing = state.favoriteEditMode
  $('#favorite-dialog-title').textContent = t(editing ? '编辑收藏' : favorite.scope === 'selection' ? '收藏选中内容' : '收藏这条回复')
  $('#favorite-source-label').textContent = `${favorite.backend === 'opencode' ? 'OpenCode' : 'Codex'} · ${favorite.threadTitle || t('未命名会话')}`
  $('#favorite-answer-length').textContent = t('{count} 字', { count: [...favorite.content].length.toLocaleString(getLocale()) })
  $('#favorite-answer-preview').innerHTML = renderMarkdown(favorite.content)
  $('#favorite-title').value = favorite.title || autoFavoriteTitle(favorite.content)
  $('#favorite-tags').value = (favorite.tags || []).join(', ')
  $('#favorite-note').value = favorite.note || ''
  $('#favorite-include-question').checked = Boolean(favorite.question)
  $('#favorite-question-option').classList.toggle('hidden', editing && !favorite.question)
  $('#save-favorite').textContent = t(editing ? '保存修改' : '保存到收藏')
  $('#favorite-error').classList.add('hidden')
  renderFavoriteQuestionOption()
  $('#favorite-dialog').showModal()
  setTimeout(() => $('#favorite-title').focus(), 30)
}

function renderFavoriteQuestionOption() {
  const favorite = state.pendingFavorite
  if (!favorite) return
  const included = $('#favorite-include-question').checked && Boolean(favorite.question)
  $('#favorite-question-preview').classList.toggle('hidden', !included)
  $('#favorite-question-preview').textContent = included ? favorite.question : ''
}

function closeFavoriteDialog() {
  $('#favorite-dialog').close()
  state.pendingFavorite = null
  state.favoriteEditMode = false
  state.pendingSelection = null
  window.getSelection()?.removeAllRanges()
}

async function saveFavorite(event) {
  event.preventDefault()
  if (!state.pendingFavorite) return
  const favorite = {
    ...state.pendingFavorite,
    title: $('#favorite-title').value.trim(),
    question: $('#favorite-include-question').checked ? state.pendingFavorite.question : '',
    tags: normalizeFavoriteTags($('#favorite-tags').value),
    note: $('#favorite-note').value.trim(),
  }
  const error = $('#favorite-error')
  if (!favorite.title) {
    error.textContent = '请填写收藏标题。'
    error.classList.remove('hidden')
    return
  }
  try {
    const updating = state.favoriteEditMode
    const saved = await favoriteRequest(
      updating ? `/studio/favorites/${encodeURIComponent(favorite.id)}` : '/studio/favorites',
      { method: updating ? 'PUT' : 'POST', body: JSON.stringify(favorite) },
    )
    closeFavoriteDialog()
    state.selectedFavorite = saved
    await loadFavorites()
    toast(updating ? '收藏已更新' : '已保存到全局收藏')
    if (updating) await openFavoriteDetail(saved.id)
  } catch (requestError) {
    error.textContent = requestError.message
    error.classList.remove('hidden')
  }
}

async function openFavoriteDetail(id) {
  const favorite = await favoriteRequest(`/studio/favorites/${encodeURIComponent(id)}`)
  state.selectedFavorite = favorite
  $('#favorite-detail-backend').textContent = favorite.backend
  $('#favorite-detail-backend').className = `favorite-backend-pill ${favorite.backend}`
  $('#favorite-detail-title').textContent = favorite.title
  $('#favorite-detail-source').textContent = `${favorite.threadTitle || t('未命名会话')} · ${favorite.projectPath || t('未记录项目目录')} · ${formatFavoriteDate(favorite.createdAt)}`
  $('#favorite-detail-question-section').classList.toggle('hidden', !favorite.question)
  $('#favorite-detail-question').textContent = favorite.question || ''
  $('#favorite-detail-answer').innerHTML = renderMarkdown(favorite.content)
  $('#favorite-detail-note-section').classList.toggle('hidden', !favorite.note)
  $('#favorite-detail-note').textContent = favorite.note || ''
  $('#favorite-detail-tags').innerHTML = (favorite.tags || []).map((tag) => `<span>${escapeHtml(tag)}</span>`).join('')
  $('#favorite-detail-dialog').showModal()
}

function closeFavoriteDetail() {
  $('#favorite-detail-dialog').close()
  state.selectedFavorite = null
}

async function copySelectedFavorite() {
  if (!state.selectedFavorite) return
  await navigator.clipboard.writeText(favoriteCopyText(state.selectedFavorite))
  toast('收藏内容已复制')
}

function editSelectedFavorite() {
  if (!state.selectedFavorite) return
  const favorite = { ...state.selectedFavorite, tags: [...(state.selectedFavorite.tags || [])] }
  $('#favorite-detail-dialog').close()
  state.favoriteEditMode = true
  state.pendingFavorite = favorite
  populateFavoriteDialog(favorite)
}

async function deleteSelectedFavorite() {
  const favorite = state.selectedFavorite
  if (!favorite || !confirm(t('删除收藏“{title}”？', { title: favorite.title }))) return
  await favoriteRequest(`/studio/favorites/${encodeURIComponent(favorite.id)}`, { method: 'DELETE' })
  closeFavoriteDetail()
  await loadFavorites()
  toast('收藏已删除')
}

async function openSelectedFavoriteSource() {
  const favorite = state.selectedFavorite
  if (!favorite) return
  closeFavoriteDetail()
  closeFavoritesRail()
  if (state.backend !== favorite.backend) {
    await switchBackend(favorite.backend)
    await waitFor(() => state.ready, 12_000)
  }
  if (!state.threads.some((thread) => thread.id === favorite.threadId)) {
    if (state.ready) await loadThreads()
  }
  if (!state.threads.some((thread) => thread.id === favorite.threadId)) {
    throw new Error('原会话当前不在会话列表中，可能已归档或删除。收藏内容仍然完整保留。')
  }
  await selectThread(favorite.threadId, { force: true })
  const element = renderedItem(favorite.turnId, favorite.itemId)
  if (!element) {
    toast('已返回原会话，但历史中没有找到原消息锚点', 'error')
    return
  }
  transcriptScrollFollower.pause()
  element.classList.add('favorite-source-highlight')
  element.scrollIntoView({ behavior: 'smooth', block: 'center' })
  setTimeout(() => element.classList.remove('favorite-source-highlight'), 2400)
}

function syncFavoriteButtons() {
  $('#transcript')?.querySelectorAll('[data-favorite-message]').forEach((button) => {
    const item = button.closest('[data-turn-id][data-item-id]')
    const favorite = item && favoriteForSource(state.backend, state.selectedId, item.dataset.turnId, item.dataset.itemId)
    button.classList.toggle('active', Boolean(favorite))
    button.setAttribute('aria-pressed', String(Boolean(favorite)))
    button.title = favorite ? '已收藏，点击查看' : '收藏这条回复'
    button.setAttribute('aria-label', button.title)
    button.querySelector('span').textContent = favorite ? '★' : '☆'
    button.querySelector('b').textContent = favorite ? '已收藏' : '收藏'
    item?.classList.toggle('favorited', Boolean(favorite))
  })
}

function formatFavoriteDate(value) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return formatLocalizedDate(date, { year: 'numeric', month: 'short', day: 'numeric' })
}

function waitFor(predicate, timeoutMs) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now()
    const check = () => {
      if (predicate()) resolve()
      else if (Date.now() - startedAt >= timeoutMs) reject(new Error('等待后端切换超时'))
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
  state.typography = normalizeTypography({ ...typographyDefaults, ...(saved.typography || {}) })
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
  state.backend = saved.selectedBackend === 'opencode' ? 'opencode' : 'codex'
  state.selectedByBackend = {
    codex: null,
    opencode: null,
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

function openRouterDialog() {
  closeActionMenus()
  state.routerEditor = {
    fallbacks: state.router.fallbacks.map((entry) => ({ ...entry })),
  }
  renderManagedRouterStatus()
  $('#router-error').classList.add('hidden')
  renderRouterFallbacks()
  $('#router-dialog').showModal()
  $('.router-dialog-body').scrollTop = 0
}

function renderManagedRouterStatus() {
  const backend = state.router.controllerBackend
  const id = state.router.controllers[backend]
  const managed = state.threadsByBackend[backend]?.find((thread) => thread.id === id)
  $('#managed-router-status').textContent = managed
    ? t('{backend} · 已创建并持续复用 · {title}', { backend: backendDescriptor(backend).name, title: threadTitle(managed) })
    : t('{backend} · 首次保存时由 Studio 自动创建', { backend: backendDescriptor(backend).name })
}

function closeRouterDialog() {
  $('#router-dialog').close()
  state.routerEditor = null
}

function captureRouterFallbacks() {
  if (!state.routerEditor) return
  state.routerEditor.fallbacks = $$('#router-fallbacks .router-fallback-card').map((row) => ({
    sessionKey: row.querySelector('.router-fallback-session').value,
    condition: row.querySelector('.router-fallback-condition').value.trim() || DEFAULT_FALLBACK_CONDITION,
  }))
}

function routerTargetCatalogs() {
  return Object.fromEntries(Object.entries(sidebarThreadCatalogs()).map(([backend, threads]) => [
    backend,
    (threads || []).filter((thread) => !isSessionDirectoryHidden(
      thread.cwd,
      state.hiddenSessionDirectories,
      state.sessionDirectoryIgnore,
    )),
  ]))
}

function routerFallbackTargets() {
  const controllerKeys = new Set(Object.entries(state.router.controllers).map(([backend, id]) => sessionRefKey(backend, id)))
  return Object.entries(routerTargetCatalogs()).flatMap(([backend, threads]) => (threads || []).flatMap((thread) => {
    const key = sessionRefKey(backend, thread.id)
    return key && !controllerKeys.has(key) && !thread.archived && !thread.ephemeral
      ? [{ key, backend, thread }]
      : []
  }))
}

function renderRouterFallbacks({ capture = false } = {}) {
  if (!state.routerEditor) return
  if (capture) captureRouterFallbacks()
  const targets = routerFallbackTargets()
  const options = (selected) => {
    const known = targets.some((target) => target.key === selected)
    return `<option value="">${t('选择 fallback 会话')}</option>${!known && selected ? `<option value="${escapeHtml(selected)}" selected>${t('已不可用')} · ${escapeHtml(selected)}</option>` : ''}${targets.map(({ key, backend, thread }) => `<option value="${escapeHtml(key)}"${key === selected ? ' selected' : ''}>[${backend === 'codex' ? 'CX' : 'OC'}] ${escapeHtml(threadTitle(thread))} — ${escapeHtml(thread.cwd || t('未记录项目目录'))}</option>`).join('')}`
  }
  const container = $('#router-fallbacks')
  container.innerHTML = state.routerEditor.fallbacks.length
    ? state.routerEditor.fallbacks.map((entry, index) => `<section class="router-fallback-card" data-router-fallback-index="${index}">
      <header><strong>${t('Fallback 目标 {index}', { index: index + 1 })}</strong><button class="icon-button router-remove-fallback" type="button" title="${t('移除 fallback')}" aria-label="${t('移除 fallback')}">×</button></header>
      <label class="field"><span>${t('目标会话')}</span><select class="router-fallback-session">${options(entry.sessionKey)}</select></label>
      <label class="field"><span>${t('Fallback 条件')}</span><textarea class="router-fallback-condition" rows="2">${escapeHtml(entry.condition || DEFAULT_FALLBACK_CONDITION)}</textarea></label>
    </section>`).join('')
    : `<div class="router-fallback-empty"><strong>${t('未配置 fallback target')}</strong><small>${t('没有精确匹配时，Router 将选择最接近的普通会话。')}</small></div>`
  container.querySelectorAll('.router-remove-fallback').forEach((button) => button.addEventListener('click', () => {
    captureRouterFallbacks()
    state.routerEditor.fallbacks.splice(Number(button.closest('.router-fallback-card').dataset.routerFallbackIndex), 1)
    renderRouterFallbacks()
  }))
  $('#router-add-fallback').disabled = state.routerEditor.fallbacks.length >= 3 || !targets.length
}

function addRouterFallback() {
  if (!state.routerEditor || state.routerEditor.fallbacks.length >= 3) return
  captureRouterFallbacks()
  const used = new Set(state.routerEditor.fallbacks.map((entry) => entry.sessionKey))
  const target = routerFallbackTargets().find((entry) => !used.has(entry.key))
  state.routerEditor.fallbacks.push({ sessionKey: target?.key || '', condition: DEFAULT_FALLBACK_CONDITION })
  renderRouterFallbacks()
}

async function saveRouterSettings(event) {
  event.preventDefault()
  captureRouterFallbacks()
  const fallbacks = (state.routerEditor?.fallbacks || []).filter((entry) => entry.sessionKey)
  if (new Set(fallbacks.map((entry) => entry.sessionKey)).size !== fallbacks.length) {
    $('#router-error').textContent = t('同一个会话不能重复配置为 fallback。')
    $('#router-error').classList.remove('hidden')
    return
  }
  const button = $('#router-form .primary-button')
  button.disabled = true
  $('#router-error').classList.add('hidden')
  try {
    const controllerBackend = state.router.controllerBackend
    const threadId = await ensureManagedRouterSession(controllerBackend)
    const controllers = { ...state.router.controllers, [controllerBackend]: threadId }
    const previousRouter = state.router
    state.router = normalizeThreadRouter({ controllerBackend, controllers, fallbacks })
    try { await persistPreferences() } catch (error) {
      state.router = previousRouter
      throw error
    }
    closeRouterDialog()
    renderThreadList()
    renderWorkspace()
    toast(t('Router 设置已保存'))
  } catch (error) {
    $('#router-error').textContent = error.message
    $('#router-error').classList.remove('hidden')
  } finally { button.disabled = false }
}

async function ensureManagedRouterSession(backend = state.router.controllerBackend) {
  if (!sessionDispatch.supports(backend)) throw new Error(t('Router 后端当前不可用。'))
  const cwd = (await loadBackendInfo('codex'))?.routerWorkspace
  if (!cwd) throw new Error(t('无法确定 Studio Router 的工作目录。'))
  const routerId = state.router.controllers[backend]
  const existing = managedRouterThread(state.threadsByBackend[backend], routerId, cwd)
  if (existing) return existing.id
  if (routerId) {
    try {
      const result = await dispatchBackendRpc(backend, 'thread/read', { threadId: routerId, includeTurns: false })
      const recoveredCatalog = recoverManagedRouterCatalog(state.threadsByBackend[backend], routerId, cwd, result?.thread)
      const recovered = managedRouterThread(recoveredCatalog, routerId, cwd)
      if (recovered) {
        state.threadsByBackend[backend] = recoveredCatalog
        if (backend === state.backend) state.threads = recoveredCatalog
        return recovered.id
      }
    } catch (error) {
      throw new Error(t('无法读取已配置的 Router 会话；为避免重复创建，Studio 将保留现有 Router ID。{message}', { message: error.message }))
    }
    throw new Error(t('已配置的 Router 会话与专用工作目录不匹配；Studio 不会自动创建替代会话。'))
  }
  if (!shouldCreateManagedRouter(routerId)) throw new Error(t('Router ID 已存在；Studio 不会自动创建替代会话。'))
  const result = await dispatchBackendRpc(backend, 'thread/start', {
    cwd,
    name: 'Thread Router',
    approvalPolicy: 'never',
    sandbox: 'read-only',
  })
  if (!result?.thread?.id) throw new Error(t('无法创建系统 Router 会话。'))
  if (backend === 'codex') await dispatchBackendRpc(backend, 'thread/name/set', { threadId: result.thread.id, name: 'Thread Router' })
  mergeThreadIntoCatalog(backend, { ...result.thread, name: 'Thread Router' })
  state.router = normalizeThreadRouter({
    ...state.router,
    controllers: { ...state.router.controllers, [backend]: result.thread.id },
  })
  persistPreferences()
  return result.thread.id
}

function openSettings() {
  populateSettingsForm()
  $('#settings-dialog').showModal()
}

function populateSettingsForm() {
  $('#language-select').value = state.language
  $('#theme-select').value = state.theme
  $('#content-width').value = state.contentWidth
  $('#ui-font-family').value = state.typography.uiFontFamily
  $('#ui-font-weight').value = String(state.typography.uiFontWeight)
  $('#workspace-font-family').value = state.typography.workspaceFontFamily
  $('#workspace-font-size').value = String(state.typography.workspaceFontSize)
  $('#code-font-family').value = state.typography.codeFontFamily
  $('#code-font-size').value = String(state.typography.codeFontSize)
  $('#code-font-weight').value = String(state.typography.codeFontWeight)
  $('#high-contrast').checked = state.typography.highContrast
  $('#desktop-notifications').checked = state.desktopNotifications
  $('#wsl-settings').classList.toggle('hidden', state.hostPlatform !== 'windows')
  $('#wsl-distribution').value = state.wsl.distribution
  $('#wsl-user').value = state.wsl.user
  $('#wsl-codex-binary').value = state.wsl.codexBinary
  $('#wsl-opencode-binary').value = state.wsl.opencodeBinary
  $('#annotation-template').value = state.annotationPromptTemplate
  $('#settings-error').classList.add('hidden')
}

async function saveSettings(event) {
  event.preventDefault()
  const template = $('#annotation-template').value.trim()
  if (!template.includes('{{annotations}}')) {
    $('#settings-error').textContent = '批注模板必须包含 {{annotations}}。'
    $('#settings-error').classList.remove('hidden')
    return
  }
  const previousLocale = getLocale()
  state.annotationPromptTemplates[previousLocale] = template.slice(0, 32000)
  state.language = normalizeLanguage($('#language-select').value)
  setLanguage(state.language)
  state.theme = $('#theme-select').value === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth($('#content-width').value)
  state.typography = normalizeTypography({
    uiFontFamily: $('#ui-font-family').value.trim(),
    uiFontWeight: Number($('#ui-font-weight').value),
    workspaceFontFamily: $('#workspace-font-family').value.trim(),
    workspaceFontSize: Number($('#workspace-font-size').value),
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
    toast(t('WSL 设置已保存，重启 Studio 后生效'))
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
    toast(t('当前会话没有项目目录。'), 'error')
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
  $('#environment-profile-status').textContent = t('正在读取')
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
  status.textContent = t(profile?.configured ? '已配置' : '默认环境')
  renderEnvironmentSecretNames()
  $('#environment-advanced').open = policy === 'enabled'
    || Boolean(profile?.allowedHosts?.length)
    || Boolean(Object.keys(profile?.cacheVariables || {}).length)
  updateEnvironmentDraftSummary()
}

function renderEnvironmentSecretNames() {
  const names = Array.isArray(environmentDialogProfile?.secretNames) ? environmentDialogProfile.secretNames : []
  $('#environment-secret-names').innerHTML = names.length
    ? names.map((name) => `<button class="environment-secret-chip" type="button" data-secret-name="${escapeHtml(name)}" aria-pressed="${environmentSecretRemovals.has(name)}" title="${escapeHtml(t('点击标记为删除；再次点击可撤销'))}">${escapeHtml(name)}</button>`).join('')
    : `<span class="environment-secret-empty">${escapeHtml(t('尚未保存 Secret'))}</span>`
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
    ? `${policy} · ${cacheNames.size} ${t('缓存项')}`
    : policy
}

async function saveProjectEnvironment(event) {
  event.preventDefault()
  const button = $('#save-environment')
  button.disabled = true
  button.textContent = t('正在保存…')
  $('#environment-error').classList.add('hidden')
  try {
    await saveEnvironmentProfile()
    closeEnvironmentDialog()
    toast(t('项目环境已保存'))
  } catch (error) {
    $('#environment-error').textContent = error.message
    $('#environment-error').classList.remove('hidden')
  } finally {
    button.disabled = false
    button.textContent = t('保存项目环境')
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
  action.title = t(root ? '配置当前项目的环境变量、Secret、网络与缓存' : '当前会话没有项目目录')
  const configured = Boolean(root && state.environmentProfile?.configured && state.environmentProfile.root === root)
  $('#project-environment-indicator').classList.toggle('hidden', !configured)
}

async function applyEnvironmentToCodex(root) {
  if (state.backend !== 'codex' || !state.selectedId) return
  const applied = await gatewayFetch('/studio/environment/apply', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, threadId: state.selectedId }),
  })
  const result = await applied.json().catch(() => null)
  if (!applied.ok) throw new Error(result?.error?.message || `HTTP ${applied.status}`)
}

function resetSettings() {
  state.language = 'system'
  setLanguage(state.language)
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
  renderFavoritesRail()
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
  root.style.setProperty('--ui-font-weight', state.typography.uiFontWeight)
  root.style.setProperty('--workspace-font-family', state.typography.workspaceFontFamily)
  root.style.setProperty('--workspace-font-size', `${state.typography.workspaceFontSize}px`)
  root.style.setProperty('--code-font-family', state.typography.codeFontFamily)
  root.style.setProperty('--code-font-size', `${state.typography.codeFontSize}px`)
  root.style.setProperty('--code-font-weight', state.typography.codeFontWeight)
  workspaceTools.refreshTypography()
  resetMermaidRendering()
}

function backendStatusView(backend) {
  const status = state.backendStates[backend] || { kind: 'idle', label: backendDescriptor(backend).name, caption: '按需连接' }
  const info = state.backendInfos[backend]
  if (backend === state.backend) return status
  if (info?.error) return { kind: 'error', label: '不可用', caption: info.error }
  if (info?.reachable) return { kind: 'online', label: '可用', caption: '按需连接会话事件' }
  return { kind: 'idle', label: '按需连接', caption: '尚未选择该后端会话' }
}

function openConnectionsDialog() {
  closeActionMenus()
  renderConnectionsDialog()
  $('#connections-dialog').showModal()
  refreshBackendInformation()
}

function renderConnectionsDialog() {
  $('#connections-dialog-content').innerHTML = ['codex', 'opencode'].map((backend) => {
    const descriptor = backendDescriptor(backend)
    const status = backendStatusView(backend)
    const info = state.backendInfos[backend] || {}
    return `<section class="connection-card ${backend}">
      <span class="connection-monogram">${backend === 'codex' ? 'CX' : 'OC'}</span>
      <span class="connection-copy"><strong>${descriptor.name}</strong><small>${escapeHtml(info.binary || descriptor.binary)} · ${escapeHtml(info.transport || descriptor.transport)}</small></span>
      <span class="connection-state ${escapeHtml(status.kind)}">${escapeHtml(t(status.label))}</span>
    </section>`
  }).join('')
}

function openBackendDialog() {
  closeActionMenus()
  renderBackendDialog()
  $('#backend-dialog').showModal()
  refreshBackendInformation()
}

function renderBackendDialog() {
  const appInfo = Object.values(state.backendInfos).find((info) => info?.appName) || {}
  const backendSections = ['codex', 'opencode'].map((backend) => {
    const descriptor = backendDescriptor(backend)
    const info = state.backendInfos[backend] || {}
    const status = backendStatusView(backend)
    return `<section class="about-section">
      <header>${descriptor.name}</header>
      <div class="detail-row"><span>${t('状态')}</span><strong>${escapeHtml(t(status.label))}</strong></div>
      <div class="detail-row"><span>${t('可执行文件')}</span><strong>${escapeHtml(info.binary || descriptor.binary)}</strong></div>
      <div class="detail-row"><span>${t('后端版本')}</span><strong>${escapeHtml(info.backendVersion || '—')}</strong></div>
      <div class="detail-row"><span>${t('协议')}</span><strong>${escapeHtml(info.protocol || descriptor.protocol)}</strong></div>
      <div class="detail-row"><span>${t('传输')}</span><strong>${escapeHtml(info.transport || descriptor.transport)}</strong></div>
      <div class="detail-row"><span>${t('执行环境')}</span><strong>${escapeHtml(info.executionEnvironment === 'wsl' ? `WSL · ${info.wslDistribution || t('默认 Distribution')}` : t('本机'))}</strong></div>
    </section>`
  }).join('')
  $('#backend-dialog-content').innerHTML = `<section class="about-section">
    <header>${t('应用')}</header>
    <div class="detail-row"><span>${t('应用')}</span><strong>${escapeHtml(appInfo.appName || 'Codex Thread Studio')}</strong></div>
    <div class="detail-row"><span>${t('版本')}</span><strong>v${escapeHtml(appInfo.appVersion || 'unknown')}</strong></div>
    <div class="detail-row"><span>${t('运行模式')}</span><strong>${t('本机工作区')}</strong></div>
  </section>${backendSections}`
}

function refreshBackendInformation() {
  Promise.all(['codex', 'opencode'].map((backend) => loadBackendInfo(backend))).then(() => {
    if ($('#connections-dialog').open) renderConnectionsDialog()
    if ($('#backend-dialog').open) renderBackendDialog()
  }).catch((error) => console.warn('Unable to refresh backend information', error))
}

function setBackendState(kind, label, caption) {
  state.backendStates[state.backend] = { kind, label, caption }
  const connection = $('#thread-connection')
  if (connection) {
    connection.className = `thread-connection ${kind}`
    const copy = kind === 'online' ? '已连接' : kind === 'checking' ? '正在连接…' : '未连接'
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
  if (message.method === 'thread/status/changed' && message.params?.threadId === thread.id) thread.status = message.params.status
  renderWorkspace()
}

function threadTitle(thread) { return thread?.name || thread?.preview || basename(thread?.cwd) || thread?.id || t('Codex 会话') }
function basename(path) { return String(path || '').split(/[\\/]/).filter(Boolean).at(-1) || '' }
function shortId(value) { const text = String(value || ''); return text.length > 12 ? `${text.slice(0, 8)}…` : text }
function threadSourceLabel(source) {
  if (typeof source === 'string') return source
  if (!source || typeof source !== 'object') return ''
  return source.type || source.kind || Object.keys(source)[0] || ''
}
function threadStatus(thread) { return thread?.status?.type || thread?.status || 'notLoaded' }
function statusLabel(status) {
  return t(({ active: '运行中', running: '运行中', inProgress: '执行中', idle: '空闲', notLoaded: '未加载', completed: '完成', interrupted: '已停止', failed: '失败', systemError: '异常', declined: '已拒绝' })[status] || status || '未知')
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
  return {
    uiFontFamily: String(value.uiFontFamily || typographyDefaults.uiFontFamily).slice(0, 512),
    uiFontWeight: weights.includes(Number(value.uiFontWeight)) ? Number(value.uiFontWeight) : 400,
    workspaceFontFamily: String(value.workspaceFontFamily || typographyDefaults.workspaceFontFamily).slice(0, 512),
    workspaceFontSize: Math.min(20, Math.max(11, Number(value.workspaceFontSize) || 14)),
    codeFontFamily: String(value.codeFontFamily || typographyDefaults.codeFontFamily).slice(0, 512),
    codeFontSize: Math.min(20, Math.max(11, Number(value.codeFontSize) || 13)),
    codeFontWeight: weights.includes(Number(value.codeFontWeight)) ? Number(value.codeFontWeight) : 400,
    highContrast: Boolean(value.highContrast),
  }
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
