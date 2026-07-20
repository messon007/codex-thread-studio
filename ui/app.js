import {
  applyCodexNotification,
  createCodexViewModel,
  hydrateCodexThread,
  resolveCodexApproval,
  textFromUserContent,
} from './codex-native.mjs'
import {
  composerTrigger,
  fuzzyFileLabel,
  matchingSkills,
  matchingSlashCommands,
  replaceComposerTrigger,
  selectedFileReference,
  selectedSkillReference,
  shellCommandFromComposer,
  transcriptUpdateKind,
} from './composer-tools.mjs'
import { marked } from './vendor/marked.esm.js'
import {
  activeTurnAtMarker,
  turnNavigationLabel,
  turnPromptPreview,
} from './turn-navigator.mjs'
import DOMPurify from './vendor/purify.es.mjs'

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
  codeFontFamily: '"JetBrains Mono", "SFMono-Regular", Consolas, monospace',
  codeFontSize: 14,
  codeFontWeight: 500,
  highContrast: true,
})

const annotationPromptDefault = `请根据下面引用的 Codex 输出和我的批注进行回应。请逐项处理，不要遗漏；若需要修改代码，请先说明你对每条意见的理解，再继续执行。

{{annotations}}

{{additional}}`

const state = {
  socket: null,
  socketGeneration: 0,
  reconnectTimer: null,
  ready: false,
  backendInfo: null,
  requestId: 0,
  pending: new Map(),
  threads: [],
  selectedId: null,
  search: '',
  model: createCodexViewModel(),
  theme: 'light',
  contentWidth: 'comfortable',
  typography: { ...typographyDefaults },
  annotationDrafts: {},
  annotationAdditional: {},
  annotationPromptTemplate: annotationPromptDefault,
  pendingSelection: null,
  composerMenu: { type: null, trigger: null, options: [], selected: 0, generation: 0 },
  skillCatalog: { cwd: null, skills: [], request: null, loaded: false },
  turnOptions: {},
  pendingSkills: {},
}

let preferencesReady = false
let preferencesWriteChain = Promise.resolve()
let annotationPersistTimer = null
let transcriptFrame = null
const dirtyStreamItems = new Map()
let composerSearchTimer = null
let turnNavigatorFrame = null

document.addEventListener('DOMContentLoaded', () => init().catch(showError))

async function init() {
  bindUI()
  await loadPreferences()
  applyAppearance()
  await loadBackendInfo()
  connectAppServer()
}

function bindUI() {
  $('#new-thread').addEventListener('click', openNewThreadDialog)
  $('#empty-new-thread').addEventListener('click', openNewThreadDialog)
  $('#close-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#cancel-new-thread').addEventListener('click', closeNewThreadDialog)
  $('#new-thread-form').addEventListener('submit', createThread)
  $('#thread-search').addEventListener('input', (event) => {
    state.search = event.target.value.trim().toLowerCase()
    renderThreadList()
  })
  $('#refresh-thread').addEventListener('click', refreshSelectedThread)
  $('#stop-thread').addEventListener('click', interruptTurn)
  $('#rename-thread').addEventListener('click', openRenameThreadDialog)
  $('#rename-thread-form').addEventListener('submit', renameSelectedThread)
  $('#close-rename-thread').addEventListener('click', closeRenameThreadDialog)
  $('#cancel-rename-thread').addEventListener('click', closeRenameThreadDialog)
  $('#fork-thread').addEventListener('click', forkSelectedThread)
  $('#archive-thread').addEventListener('click', archiveSelectedThread)
  $('#delete-thread').addEventListener('click', deleteSelectedThread)
  $('#retry-native').addEventListener('click', connectAppServer)
  $('#composer-form').addEventListener('submit', sendComposer)
  $('#composer-input').addEventListener('input', handleComposerInput)
  $('#composer-input').addEventListener('keydown', handleComposerKeydown)
  $('#composer-menu').addEventListener('mousedown', (event) => event.preventDefault())
  $('#composer-menu').addEventListener('click', handleComposerMenuClick)
  $('#interrupt-turn').addEventListener('click', interruptTurn)
  $('#turn-navigator-list').addEventListener('click', handleTurnNavigatorClick)
  $('#transcript').addEventListener('scroll', scheduleTurnNavigatorSync, { passive: true })
  $('#transcript').addEventListener('mouseup', captureTranscriptSelection)
  $('#transcript').addEventListener('click', handleTranscriptClick)
  $('#comment-selection').addEventListener('mousedown', (event) => event.preventDefault())
  window.addEventListener('resize', scheduleTurnNavigatorSync)
  $('#comment-selection').addEventListener('click', openAnnotationFromSelection)
  $('#selection-popover').addEventListener('mousedown', (event) => event.preventDefault())
  $('#selection-popover').addEventListener('click', openAnnotationFromSelection)
  $('#open-annotation-rail').addEventListener('click', openAnnotationRail)
  $('#close-annotation-rail').addEventListener('click', closeAnnotationRail)
  $('#annotation-form').addEventListener('submit', addAnnotation)
  $('#close-annotation-dialog').addEventListener('click', closeAnnotationDialog)
  $('#cancel-annotation').addEventListener('click', closeAnnotationDialog)
  $('#clear-annotations').addEventListener('click', clearAnnotations)
  $('#insert-annotations').addEventListener('click', insertAnnotations)
  $('#annotation-additional').addEventListener('input', saveAnnotationAdditional)
  $('#settings-button').addEventListener('click', openSettings)
  $('#close-settings').addEventListener('click', () => $('#settings-dialog').close())
  $('#settings-form').addEventListener('submit', saveSettings)
  $('#reset-settings').addEventListener('click', resetSettings)
  $('#backend-details').addEventListener('click', openBackendDialog)
  $('#close-backend').addEventListener('click', () => $('#backend-dialog').close())
  $('#close-command').addEventListener('click', () => $('#command-dialog').close())

  document.addEventListener('mousedown', (event) => {
    if (!event.target.closest('#selection-popover, #comment-selection')) hideSelectionPopover()
  })
  document.addEventListener('keydown', (event) => {
    const modifier = event.ctrlKey || event.metaKey
    if (modifier && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      openNewThreadDialog()
    } else if (event.key === '/' && !isTypingTarget(event.target)) {
      event.preventDefault()
      $('#thread-search').focus()
    } else if (event.key === 'Escape') {
      hideSelectionPopover()
      closeAnnotationRail()
    }
  })
}

async function loadBackendInfo() {
  try {
    const response = await fetch('/studio/codex', { cache: 'no-store' })
    state.backendInfo = await response.json()
  } catch (error) {
    state.backendInfo = { binary: 'codex', protocol: 'Codex App Server v2', transport: 'stdio JSONL' }
  }
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
  const socket = new WebSocket(`${protocol}//${location.host}/ws/codex`)
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

function cleanupSocket() {
  if (!state.socket) return
  state.socket.onclose = null
  state.socket.close()
  state.socket = null
}

function handleAppServerMessage(message) {
  if (message.method === 'studio/appServer/status') {
    const status = message.params?.state
    if (status === 'ready') {
      const firstReady = !state.ready
      state.ready = true
      setBackendState('online', 'Codex App Server', '原生结构化连接')
      $('#native-connection').textContent = '已连接'
      setNativeError(null)
      if (firstReady) loadThreads().catch(showError)
    } else if (status === 'starting') {
      setBackendState('checking', '正在启动 Codex', message.params?.binary || 'App Server')
    } else if (status === 'error' || status === 'stopped') {
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
      toast(`界面错过了 ${skipped} 条 App Server 事件`, 'error')
      return
    }
    setNativeError(`界面错过了 ${skipped} 条 App Server 事件，正在从 Codex 重新同步当前会话…`)
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
    const pending = state.pending.get(String(message.id))
    if (!pending) return
    state.pending.delete(String(message.id))
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(message.error.message || JSON.stringify(message.error)))
    else pending.resolve(message.result)
    return
  }

  if (message.method === 'thread/started' && message.params?.thread) {
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
    if (message.method === 'thread/deleted') {
      delete state.annotationDrafts[threadId]
      delete state.annotationAdditional[threadId]
    }
    if (state.selectedId === threadId) {
      state.selectedId = null
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

  if (message.id != null && message.method) {
    if (!applyCodexNotification(state.model, message)) {
      sendRaw({ id: message.id, error: { code: -32601, message: `Studio does not support ${message.method}` } })
      toast(`Codex 请求了尚未支持的交互：${message.method}`, 'error')
      return
    }
    renderTranscript(false)
    return
  }

  if (applyCodexNotification(state.model, message)) {
    const updateKind = transcriptUpdateKind(message.method)
    if (updateKind === 'stream') queueStreamingItemPatch(message.params)
    else if (updateKind === 'item') replaceCompletedItem(message.params)
    else if (updateKind === 'full') renderTranscript(message.method === 'item/started')
    if (updateKind === 'metadata') renderUsage()
    renderComposerState()
    updateSelectedThreadStatus(message)
  }
}

function rpc(method, params = {}, timeoutMs = 30_000) {
  if (!state.ready || state.socket?.readyState !== WebSocket.OPEN) return Promise.reject(new Error('Codex App Server 尚未就绪'))
  const id = ++state.requestId
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      state.pending.delete(String(id))
      reject(new Error(`${method} 请求超时`))
    }, timeoutMs)
    state.pending.set(String(id), { resolve, reject, timer, method })
    sendRaw({ id, method, params })
  })
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
  state.threads = Array.isArray(result?.data) ? result.data : []
  $('#thread-count').textContent = state.threads.length
  renderThreadList()
  const preferred = state.selectedId || state.threads.find((thread) => thread.id === state.selectedId)?.id
  const nextId = state.threads.some((thread) => thread.id === preferred) ? preferred : state.threads[0]?.id
  if (nextId) await selectThread(nextId, { force: true })
  else renderWorkspace()
}

function filteredThreads() {
  if (!state.search) return state.threads
  return state.threads.filter((thread) => [threadTitle(thread), thread.cwd, thread.id, thread.preview].join(' ').toLowerCase().includes(state.search))
}

function renderThreadList() {
  const list = $('#thread-list')
  const threads = filteredThreads()
  if (!threads.length) {
    list.innerHTML = `<div class="list-empty">${state.search ? '没有匹配的会话' : '还没有 Codex 会话'}</div>`
    return
  }
  list.innerHTML = projectGroups(threads).map(({ cwd, threads: projectThreads }) => {
    const label = basename(cwd) || '其他会话'
    const rows = projectThreads.map((thread) => {
      const status = threadStatus(thread)
      const relationship = thread.parentThreadId
        ? `子代理 · ${shortId(thread.parentThreadId)}`
        : thread.forkedFromId
          ? `Fork · ${shortId(thread.forkedFromId)}`
          : `会话树 · ${shortId(thread.sessionId || thread.id)}`
      return `<button class="thread-row${thread.id === state.selectedId ? ' active' : ''}" data-thread-id="${escapeHtml(thread.id)}">
        <span class="status-dot ${escapeHtml(status)}"></span>
        <span class="thread-copy"><strong>${escapeHtml(threadTitle(thread))}</strong><small>${escapeHtml(relationship)}</small></span>
      </button>`
    }).join('')
    return `<section class="thread-group"><header title="${escapeHtml(cwd || '未记录项目目录')}"><span><strong>${escapeHtml(label)}</strong><small>${escapeHtml(cwd || '未记录项目目录')}</small></span><b>${projectThreads.length}</b></header>${rows}</section>`
  }).join('')
  list.querySelectorAll('.thread-row').forEach((row) => row.addEventListener('click', () => selectThread(row.dataset.threadId)))
}

function projectGroups(threads) {
  const groups = new Map()
  for (const thread of threads) {
    const cwd = thread.cwd || ''
    if (!groups.has(cwd)) groups.set(cwd, [])
    groups.get(cwd).push(thread)
  }
  return [...groups].map(([cwd, groupedThreads]) => ({ cwd, threads: groupedThreads }))
}

async function selectThread(id, { force = false } = {}) {
  if (!force && state.selectedId === id) return
  const previousId = state.selectedId
  if (previousId && previousId !== id && state.ready) {
    try {
      await rpc('thread/unsubscribe', { threadId: previousId })
    } catch (error) {
      console.warn(`Could not unsubscribe from ${previousId}`, error)
    }
  }
  hideComposerMenu()
  resetStreamingPatches()
  state.selectedId = id
  state.model = createCodexViewModel()
  state.model.threadId = id
  persistPreferences()
  renderThreadList()
  renderWorkspace()
  renderTranscript(false)
  await resumeThread(id)
}

async function resumeThread(id) {
  setNativeError(null)
  $('#native-connection').textContent = '正在恢复会话…'
  try {
    const result = await rpc('thread/resume', { threadId: id })
    if (state.selectedId !== id) return
    hydrateCodexThread(state.model, result.thread)
    mergeThreadMetadata(result.thread)
    $('#native-connection').textContent = '已连接'
    renderWorkspace()
    renderTranscript(false)
  } catch (error) {
    if (state.selectedId !== id) return
    state.model.error = error.message
    state.model.status = 'failed'
    setNativeError(`无法恢复此 Codex 会话：${error.message}`)
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
    mergeThreadMetadata(result.thread)
    renderWorkspace()
    renderTranscript(false)
    if (!quiet) toast('会话已刷新')
    return true
  } catch (error) {
    if (quiet) setNativeError(`无法重新同步当前 Codex 会话：${error.message}`)
    else showError(error)
    return false
  }
}

function mergeThreadMetadata(incoming) {
  if (!incoming?.id) return
  const index = state.threads.findIndex((thread) => thread.id === incoming.id)
  if (index >= 0) state.threads[index] = { ...state.threads[index], ...incoming, turns: undefined }
  else state.threads.unshift({ ...incoming, turns: undefined })
  renderThreadList()
}

function selectedThread() {
  return state.threads.find((thread) => thread.id === state.selectedId) || null
}

function renderWorkspace() {
  const thread = selectedThread()
  const hasThread = Boolean(thread)
  $('#thread-heading').classList.toggle('hidden', !hasThread)
  $('#thread-actions').classList.toggle('hidden', !hasThread)
  $('#empty-workspace').classList.toggle('hidden', hasThread)
  $('#native-workspace').classList.toggle('hidden', !hasThread)
  if (!thread) return
  $('#thread-title').textContent = threadTitle(thread)
  $('#thread-path').textContent = thread.cwd || thread.id
  const source = threadSourceLabel(thread.source)
  const metadata = [
    { label: '会话树', value: shortId(thread.sessionId || thread.id), title: thread.sessionId || thread.id },
    thread.forkedFromId && { label: 'Fork 自', value: shortId(thread.forkedFromId), title: thread.forkedFromId },
    thread.parentThreadId && { label: '父会话', value: shortId(thread.parentThreadId), title: thread.parentThreadId },
    source && { label: '来源', value: source, title: source },
    thread.cliVersion && { label: 'CLI', value: thread.cliVersion, title: thread.cliVersion },
  ].filter(Boolean)
  $('#thread-meta').innerHTML = metadata.map((item) => `<span title="${escapeHtml(item.title)}">${escapeHtml(item.label)} <strong>${escapeHtml(item.value)}</strong></span>`).join('')
  const status = state.model.status === 'disconnected' ? threadStatus(thread) : state.model.status
  $('#thread-status').textContent = statusLabel(status)
  $('#thread-status').className = `status-badge ${status}`
  renderComposerState()
  renderAnnotationRail()
}

function resetStreamingPatches() {
  if (transcriptFrame != null) cancelAnimationFrame(transcriptFrame)
  transcriptFrame = null
  dirtyStreamItems.clear()
}

function renderTranscript(followOutput) {
  if (!state.selectedId) return
  resetStreamingPatches()
  const container = $('#transcript')
  const nearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 100
  const turns = state.model.turns || []
  container.innerHTML = turns.map((turn, index) => renderTurn(turn, index)).join('') + renderApprovals()
  bindApprovalButtons()
  renderTurnNavigator()
  if (followOutput && nearBottom) requestAnimationFrame(() => { container.scrollTop = container.scrollHeight })
  renderUsage()
}

function renderTurnNavigator() {
  const navigator = $('#turn-navigator')
  const list = $('#turn-navigator-list')
  const turns = state.model.turns || []
  if (turns.length < 2) {
    navigator.classList.add('hidden')
    list.innerHTML = ''
    return
  }

  list.innerHTML = turns.map((turn, index) => {
    const label = turnNavigationLabel(turn, index)
    const title = turnPromptPreview(turn) || `Turn ${index + 1}`
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
  const positions = [...transcript.querySelectorAll('.turn[data-turn-id]')]
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
  const target = [...transcript.querySelectorAll('.turn[data-turn-id]')]
    .find((turn) => turn.dataset.turnId === button.dataset.turnNavId)
  if (!target) return
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
  const container = $('#transcript')
  const nearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 100
  let needsFullRender = false
  for (const identity of dirtyStreamItems.values()) {
    if (!patchStreamingItem(identity.turnId, identity.itemId)) needsFullRender = true
  }
  dirtyStreamItems.clear()
  if (needsFullRender) renderTranscript(nearBottom)
  else if (nearBottom) container.scrollTop = container.scrollHeight
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
  if (!item || !element) return false
  if (item.type === 'agentMessage' || item.type === 'plan') {
    const body = element.querySelector('.markdown-body')
    if (!body) return false
    body.classList.add('streaming-markdown')
    body.textContent = item.text || ''
    return true
  }
  if (item.type === 'reasoning') {
    const body = element.querySelector('.markdown-body')
    if (!body) return false
    body.classList.add('streaming-markdown')
    body.textContent = arrayText(item.summary) || arrayText(item.content) || ''
    return true
  }
  if (item.type === 'commandExecution') {
    let output = element.querySelector('pre')
    if (!output) {
      output = document.createElement('pre')
      element.append(output)
    }
    output.textContent = item.aggregatedOutput || ''
    return true
  }
  return false
}

function replaceCompletedItem(params = {}) {
  const itemId = params.item?.id || params.itemId
  dirtyStreamItems.delete(`${params.turnId || ''}:${itemId || ''}`)
  const item = modelItem(params.turnId, itemId)
  const element = renderedItem(params.turnId, itemId)
  if (!item || !element) {
    renderTranscript(false)
    return
  }
  const template = document.createElement('template')
  template.innerHTML = renderItem(item, params.turnId)
  element.replaceWith(template.content)
  if (item.type === 'userMessage') renderTurnNavigator()
}

function renderTurn(turn, index) {
  const items = Array.isArray(turn.items) ? turn.items : []
  const content = items.map((item) => renderItem(item, turn.id)).join('')
  const error = turn.error?.message
  const result = turn.status && turn.status !== 'inProgress'
    ? `<div class="turn-result ${turn.status === 'failed' ? 'failed' : ''}">${escapeHtml(statusLabel(turn.status))}${error ? ` · ${escapeHtml(error)}` : ''}</div>`
    : ''
  return `<section class="turn" data-turn-id="${escapeHtml(turn.id || '')}">
    <div class="turn-separator">Turn ${index + 1}</div>${content || '<div class="reasoning">Codex 正在准备此 Turn…</div>'}${result}
  </section>`
}

function renderItem(item, turnId) {
  const type = item?.type || 'unknown'
  const attrs = `data-turn-id="${escapeHtml(turnId || '')}" data-item-id="${escapeHtml(item?.id || '')}"`
  if (type === 'userMessage') {
    return `<div class="message user" ${attrs}><span class="item-label">You</span>${escapeHtml(textFromUserContent(item.content) || '(非文字输入)')}</div>`
  }
  if (type === 'agentMessage' || type === 'plan') {
    return `<div class="message agent" ${attrs}><span class="item-label">Codex</span><div class="markdown-body">${renderMarkdown(item.text || '')}</div></div>`
  }
  if (type === 'reasoning') {
    const summary = arrayText(item.summary) || arrayText(item.content) || 'Codex 正在推理…'
    return `<details class="reasoning" ${attrs} open><summary>推理摘要</summary><div class="markdown-body compact-markdown">${renderMarkdown(summary)}</div></details>`
  }
  if (type === 'commandExecution') {
    const command = Array.isArray(item.command) ? item.command.join(' ') : item.command || ''
    return `<article class="item-card" ${attrs}><header><span>命令 · ${escapeHtml(command)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${item.aggregatedOutput ? `<pre>${escapeHtml(item.aggregatedOutput)}</pre>` : ''}</article>`
  }
  if (type === 'fileChange') {
    const changes = (item.changes || []).map((change) => `${change.kind || 'update'} ${change.path || ''}\n${change.diff || ''}`).join('\n\n')
    return `<article class="item-card" ${attrs}><header><span>文件修改 · ${(item.changes || []).length} 个文件</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header><pre>${escapeHtml(changes || '等待差异内容…')}</pre></article>`
  }
  if (type === 'planUpdate') {
    const rows = (item.plan || []).map((step) => `<li class="${escapeHtml(step.status || '')}">${escapeHtml(step.step || '')}</li>`).join('')
    return `<article class="item-card" ${attrs}><header><span>执行计划</span><span>${escapeHtml(item.explanation || '')}</span></header><ol class="plan-list">${rows}</ol></article>`
  }
  if (type === 'mcpToolCall' || type === 'collabToolCall' || type === 'webSearch') {
    const label = type === 'webSearch' ? `网页搜索 · ${item.query || ''}` : `${item.server || 'Tool'} · ${item.tool || type}`
    const detail = item.result || item.error || item.arguments || item.results || ''
    return `<article class="item-card" ${attrs}><header><span>${escapeHtml(label)}</span><span class="item-status ${escapeHtml(item.status || '')}">${escapeHtml(statusLabel(item.status))}</span></header>${detail ? `<pre>${escapeHtml(valueText(detail))}</pre>` : ''}</article>`
  }
  if (type === 'contextCompaction') return `<div class="reasoning" ${attrs}>Codex 已压缩较早的会话上下文。</div>`
  return `<article class="item-card" ${attrs}><header><span>${escapeHtml(type)}</span></header><pre>${escapeHtml(valueText(item))}</pre></article>`
}

function renderMarkdown(value) {
  const source = String(value || '')
  if (!source) return ''
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
    const header = document.createElement('div')
    header.className = 'markdown-code-header'
    const label = document.createElement('span')
    label.textContent = language
    const copy = document.createElement('button')
    copy.className = 'copy-code-button'
    copy.type = 'button'
    copy.textContent = '复制'
    header.append(label, copy)
    pre.replaceWith(block)
    block.append(header, pre)
  })
  template.content.querySelectorAll('table').forEach((table) => {
    const wrapper = document.createElement('div')
    wrapper.className = 'markdown-table-wrap'
    table.replaceWith(wrapper)
    wrapper.append(table)
  })
  return template.innerHTML
}

async function handleTranscriptClick(event) {
  const button = event.target.closest('.copy-code-button')
  if (!button) return
  const code = button.closest('.markdown-code-block')?.querySelector('code')
  if (!code) return
  try {
    await navigator.clipboard.writeText(code.textContent || '')
    const original = button.textContent
    button.textContent = '已复制'
    setTimeout(() => { if (button.isConnected) button.textContent = original }, 1400)
  } catch (error) {
    toast('无法复制代码', 'error')
  }
}

function renderApprovals() {
  return state.model.approvals.map((approval) => {
    const params = approval.params || {}
    const command = Array.isArray(params.command) ? params.command.join(' ') : params.command || params.reason || approval.method
    const permission = approval.method === 'item/permissions/requestApproval'
    return `<section class="turn"><article class="approval-card" data-approval-id="${escapeHtml(String(approval.id))}">
      <strong>${permission ? 'Codex 请求额外权限' : 'Codex 正在等待审批'}</strong>
      <pre>${escapeHtml(command)}${params.cwd ? `\n${escapeHtml(params.cwd)}` : ''}</pre>
      <div class="approval-actions">
        <button class="subtle-button approval-decline" type="button">拒绝</button>
        <button class="subtle-button approval-session" type="button">本会话允许</button>
        <button class="primary-button approval-accept" type="button">允许本次</button>
      </div>
    </article></section>`
  }).join('')
}

function bindApprovalButtons() {
  $$('.approval-card').forEach((card) => {
    card.querySelector('.approval-decline').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'decline'))
    card.querySelector('.approval-session').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'acceptForSession'))
    card.querySelector('.approval-accept').addEventListener('click', () => answerApproval(card.dataset.approvalId, 'accept'))
  })
}

function answerApproval(id, decision) {
  const approval = state.model.approvals.find((candidate) => String(candidate.id) === String(id))
  if (!approval) return
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
  renderTranscript(false)
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
    menu.innerHTML = `<div class="composer-menu-empty">${escapeHtml(message || empty)}</div>`
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
    return `<button id="composer-option-${index}" class="composer-option${selected ? ' selected' : ''}" type="button" role="option" aria-selected="${selected}" data-composer-index="${index}"><strong>${escapeHtml(title)}</strong><small>${escapeHtml(detail || '')}</small></button>`
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
    renderComposerMenu(`文件搜索失败：${error.message}`)
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
    renderComposerMenu(`技能读取失败：${error.message}`)
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
  state.turnOptions[state.selectedId] ||= {}
  return state.turnOptions[state.selectedId]
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
    return `<div class="command-card"><strong>${escapeHtml(model.displayName || model.model || model.id)}</strong><small>${escapeHtml(model.model || model.id)}${model.isDefault ? ' · 默认' : ''}</small>${effortOptions ? `<select aria-label="推理强度">${effortOptions}</select>` : '<span></span>'}<button class="subtle-button" type="button" data-model="${escapeHtml(model.model || model.id)}">使用</button></div>`
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
    toast(`已选择模型 ${options.model}${effort ? ` · ${effort}` : ''}`)
  }
}

function openPermissionsCommand() {
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
    if (type === 'dangerFullAccess' && !confirm('确认对后续 Turn 使用完全访问权限？')) return
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
    ['模型', options.model || thread?.model || 'Codex 默认'],
    ['推理强度', options.effort || 'Codex 默认'],
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
  const result = await rpc('review/start', { threadId: state.selectedId, target: { type: 'uncommittedChanges' }, delivery: 'inline' })
  if (result?.turn) {
    applyCodexNotification(state.model, { method: 'turn/started', params: { turn: result.turn } })
    renderTranscript(true)
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
    ? `<div class="command-list">${servers.map((server) => `<div class="command-card"><strong>${escapeHtml(server.name)}</strong><small>${Object.keys(server.tools || {}).length} 个工具 · ${server.resources?.length || 0} 个资源</small><span>${escapeHtml(valueText(server.authStatus || 'unknown'))}</span></div>`).join('')}</div>`
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
  const skillsForThread = state.pendingSkills[state.selectedId] ||= []
  if (!skillsForThread.some((candidate) => candidate.path === skill.path)) {
    skillsForThread.push({ type: 'skill', name: skill.name, path: skill.path })
  }
}

async function copyLatestAgentResponse() {
  const items = state.model.turns.flatMap((turn) => turn.items || []).reverse()
  const message = items.find((item) => (item.type === 'agentMessage' || item.type === 'plan') && item.text)
  if (!message) throw new Error('当前会话还没有可复制的 Codex 回复。')
  await navigator.clipboard.writeText(message.text)
  toast('已复制最近一条 Codex 回复')
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
  if (!handler) throw new Error(`尚未支持命令：/${action}`)
  await handler()
}

function renderComposerState() {
  const active = Boolean(state.model.activeTurnId)
  const options = currentTurnOptions()
  const shellCommand = shellCommandFromComposer($('#composer-input').value)
  const shellMode = shellCommand !== null
  $('#composer-form').classList.toggle('shell-mode', shellMode)
  $('#interrupt-turn').classList.toggle('hidden', !active)
  $('#stop-thread').classList.toggle('hidden', !active)
  $('#archive-thread').disabled = active
  $('#delete-thread').disabled = active
  $('#send-message').textContent = shellMode ? '运行' : active ? '追加意见' : '发送'
  const details = [
    options.model && `${options.model}${options.effort ? `/${options.effort}` : ''}`,
    options.sandboxPolicy?.type,
    state.pendingSkills[state.selectedId]?.length && `${state.pendingSkills[state.selectedId].length} 个技能`,
  ].filter(Boolean)
  const baseHint = shellMode
    ? active
      ? 'Shell 命令需等待当前 Turn 完成'
      : '本地 Shell · 不经过模型且不受 Turn sandbox 限制'
    : active
      ? '将通过 turn/steer 加入当前 Turn'
      : '将通过 turn/start 开始新 Turn'
  $('#composer-hint').textContent = `${baseHint}${!shellMode && details.length ? ` · ${details.join(' · ')}` : ''}`
  $('#send-message').disabled = !state.ready || !state.selectedId || (shellMode && (active || !shellCommand))
  $('#thread-status').textContent = statusLabel(state.model.status)
  $('#thread-status').className = `status-badge ${state.model.status}`
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
    try {
      await rpc('thread/shellCommand', { threadId: state.selectedId, command: shellCommand }, 120_000)
      input.value = ''
      hideComposerMenu()
      renderComposerState()
      toast('Shell 命令已交给 Codex 执行')
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
  const skillInputs = state.pendingSkills[state.selectedId] || []
  const turnInput = [{ type: 'text', text }, ...skillInputs]
  const button = $('#send-message')
  button.disabled = true
  try {
    if (state.model.activeTurnId) {
      await rpc('turn/steer', {
        threadId: state.selectedId,
        expectedTurnId: state.model.activeTurnId,
        clientUserMessageId: randomId(),
        input: turnInput,
      })
      toast('意见已加入当前 Turn')
    } else {
      const result = await rpc('turn/start', {
        threadId: state.selectedId,
        clientUserMessageId: randomId(),
        input: turnInput,
        ...currentTurnOptions(),
      })
      if (result?.turn) {
        applyCodexNotification(state.model, { method: 'turn/started', params: { turn: result.turn } })
        renderTranscript(true)
      }
    }
    input.value = ''
    state.pendingSkills[state.selectedId] = []
    hideComposerMenu()
    renderComposerState()
  } catch (error) { showError(error) }
  finally { button.disabled = false }
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
  $('#new-thread-dialog').showModal()
  setTimeout(() => ($('#new-thread-name').value ? $('#new-thread-name') : $('#new-thread-cwd')).focus(), 30)
}

function closeNewThreadDialog() { $('#new-thread-dialog').close() }

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
  const name = $('#new-thread-name').value.trim()
  const params = {
    cwd: $('#new-thread-cwd').value.trim(),
    approvalPolicy: $('#new-thread-approval').value,
    sandbox: $('#new-thread-sandbox').value,
  }
  const model = $('#new-thread-model').value.trim()
  if (model) params.model = model
  try {
    const result = await rpc('thread/start', params)
    if (name) await rpc('thread/name/set', { threadId: result.thread.id, name })
    closeNewThreadDialog()
    $('#new-thread-form').reset()
    await loadThreads()
    await selectThread(result.thread.id, { force: true })
    toast('Codex 会话已创建')
  } catch (error) {
    errorBox.textContent = error.message
    errorBox.classList.remove('hidden')
  } finally { button.disabled = false }
}

async function forkSelectedThread() {
  if (!state.selectedId) return
  try {
    const result = await rpc('thread/fork', { threadId: state.selectedId })
    await loadThreads()
    await selectThread(result.thread.id, { force: true })
    toast('已创建 Codex 会话分支')
  } catch (error) { showError(error) }
}

async function archiveSelectedThread() {
  if (!state.selectedId || !confirm('归档当前 Codex 会话？')) return
  const threadId = state.selectedId
  try {
    await rpc('thread/archive', { threadId })
    state.selectedId = null
    state.model = createCodexViewModel()
    persistPreferences()
    await loadThreads()
    toast('会话已归档')
  } catch (error) { showError(error) }
}

async function deleteSelectedThread() {
  if (!state.selectedId || !confirm('永久删除当前 Codex 会话及其持久化历史？此操作无法撤销。')) return
  const threadId = state.selectedId
  try {
    await rpc('thread/delete', { threadId })
    delete state.annotationDrafts[threadId]
    delete state.annotationAdditional[threadId]
    state.selectedId = null
    state.model = createCodexViewModel()
    persistPreferences()
    await loadThreads()
    toast('会话已删除')
  } catch (error) { showError(error) }
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
  }
  const rect = range.getBoundingClientRect()
  const popover = $('#selection-popover')
  popover.style.left = `${Math.min(window.innerWidth - 150, Math.max(8, rect.left + rect.width / 2 - 55))}px`
  popover.style.top = `${Math.max(8, rect.top - 39)}px`
  popover.classList.remove('hidden')
}

function openAnnotationFromSelection() {
  if (!state.pendingSelection?.quote) {
    captureTranscriptSelection()
    if (!state.pendingSelection?.quote) return toast('请先在 Codex 输出中选择文字', 'error')
  }
  $('#annotation-quote').textContent = state.pendingSelection.quote
  $('#annotation-comment').value = ''
  $('#annotation-error').classList.add('hidden')
  hideSelectionPopover(false)
  $('#annotation-dialog').showModal()
  setTimeout(() => $('#annotation-comment').focus(), 30)
}

function hideSelectionPopover(clear = true) {
  $('#selection-popover').classList.add('hidden')
  if (clear) state.pendingSelection = null
}

function closeAnnotationDialog() {
  $('#annotation-dialog').close()
  state.pendingSelection = null
  window.getSelection()?.removeAllRanges()
}

function currentAnnotations() {
  return state.selectedId ? state.annotationDrafts[state.selectedId] || [] : []
}

function addAnnotation(event) {
  event.preventDefault()
  const comment = $('#annotation-comment').value.trim()
  const errorBox = $('#annotation-error')
  if (!state.selectedId || !state.pendingSelection?.quote || !comment) {
    errorBox.textContent = '选中内容和意见都不能为空。'
    errorBox.classList.remove('hidden')
    return
  }
  const drafts = currentAnnotations()
  if (drafts.length >= 32) {
    errorBox.textContent = '每个会话最多保留 32 条批注。'
    errorBox.classList.remove('hidden')
    return
  }
  state.annotationDrafts[state.selectedId] = [...drafts, {
    id: randomId(),
    quote: state.pendingSelection.quote,
    comment: comment.slice(0, 16000),
    createdAt: new Date().toISOString(),
    itemId: state.pendingSelection.itemId,
    turnId: state.pendingSelection.turnId,
  }]
  persistPreferences()
  closeAnnotationDialog()
  openAnnotationRail()
  renderAnnotationRail()
  toast('批注已加入回覆草稿')
}

function openAnnotationRail() { $('#annotation-rail').classList.remove('hidden'); renderAnnotationRail() }
function closeAnnotationRail() { $('#annotation-rail').classList.add('hidden') }

function renderAnnotationRail() {
  const drafts = currentAnnotations()
  $('#annotation-count').textContent = drafts.length
  $('#draft-count').textContent = drafts.length
  $('#annotation-empty').classList.toggle('hidden', drafts.length > 0)
  $('#annotation-list').classList.toggle('hidden', drafts.length === 0)
  $('#clear-annotations').disabled = !drafts.length && !state.annotationAdditional[state.selectedId]
  $('#insert-annotations').disabled = !drafts.length
  $('#annotation-additional').value = state.selectedId ? state.annotationAdditional[state.selectedId] || '' : ''
  $('#annotation-list').innerHTML = drafts.map((draft, index) => `<article class="annotation-card" data-draft-id="${escapeHtml(draft.id)}">
    <header><span>批注 ${index + 1}${draft.turnId ? ` · ${escapeHtml(draft.turnId.slice(0, 8))}` : ''}</span><button class="annotation-delete" type="button">×</button></header>
    <blockquote>${escapeHtml(draft.quote)}</blockquote><p>${escapeHtml(draft.comment)}</p>
  </article>`).join('')
  $$('.annotation-delete').forEach((button) => button.addEventListener('click', () => deleteAnnotation(button.closest('.annotation-card').dataset.draftId)))
}

function deleteAnnotation(id) {
  if (!state.selectedId) return
  state.annotationDrafts[state.selectedId] = currentAnnotations().filter((draft) => draft.id !== id)
  if (!state.annotationDrafts[state.selectedId].length) delete state.annotationDrafts[state.selectedId]
  persistPreferences()
  renderAnnotationRail()
}

function clearAnnotations() {
  if (!state.selectedId || !confirm('清空当前会话的全部批注草稿？')) return
  delete state.annotationDrafts[state.selectedId]
  delete state.annotationAdditional[state.selectedId]
  persistPreferences()
  renderAnnotationRail()
}

function saveAnnotationAdditional(event) {
  if (!state.selectedId) return
  const value = event.target.value.slice(0, 32000)
  if (value) state.annotationAdditional[state.selectedId] = value
  else delete state.annotationAdditional[state.selectedId]
  clearTimeout(annotationPersistTimer)
  annotationPersistTimer = setTimeout(persistPreferences, 300)
}

function buildAnnotationPrompt(drafts, additional = '') {
  const annotations = drafts.map((draft, index) => {
    const anchor = [draft.turnId && `Turn ${draft.turnId}`, draft.itemId && `Item ${draft.itemId}`].filter(Boolean).join(' / ')
    const quote = draft.quote.split('\n').map((line) => `> ${line}`).join('\n')
    return `批注 ${index + 1}${anchor ? `（${anchor}）` : ''}\n引用：\n${quote}\n\n我的意见：\n${draft.comment}`
  }).join('\n\n---\n\n')
  const additionalBlock = additional.trim() ? `整体补充：\n${additional.trim()}` : ''
  return state.annotationPromptTemplate
    .replaceAll('{{annotations}}', annotations)
    .replaceAll('{{additional}}', additionalBlock)
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

function insertAnnotations() {
  const drafts = currentAnnotations()
  if (!drafts.length) return
  const prompt = buildAnnotationPrompt(drafts, state.annotationAdditional[state.selectedId] || '')
  const composer = $('#composer-input')
  composer.value = [composer.value.trim(), prompt].filter(Boolean).join('\n\n')
  closeAnnotationRail()
  composer.focus()
  toast('批注草稿已插入输入框')
}

async function loadPreferences() {
  let saved = {}
  try {
    const response = await fetch('/studio/preferences', { cache: 'no-store' })
    if (response.ok) saved = await response.json()
  } catch (error) { console.warn('Unable to load preferences', error) }
  state.theme = saved.theme === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth(saved.contentWidth)
  state.typography = normalizeTypography({ ...typographyDefaults, ...(saved.typography || {}) })
  state.selectedId = typeof saved.selectedThread === 'string' ? saved.selectedThread : null
  state.annotationDrafts = normalizeAnnotationDrafts(saved.annotationDrafts)
  state.annotationAdditional = normalizeAdditional(saved.annotationAdditional)
  state.annotationPromptTemplate = normalizeTemplate(saved.annotationPromptTemplate)
  preferencesReady = true
}

function preferencesSnapshot() {
  return {
    theme: state.theme,
    contentWidth: state.contentWidth,
    typography: state.typography,
    selectedThread: state.selectedId,
    annotationDrafts: state.annotationDrafts,
    annotationAdditional: state.annotationAdditional,
    annotationPromptTemplate: state.annotationPromptTemplate,
  }
}

function persistPreferences() {
  if (!preferencesReady) return
  const body = JSON.stringify(preferencesSnapshot())
  preferencesWriteChain = preferencesWriteChain.then(async () => {
    const response = await fetch('/studio/preferences', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
  }).catch((error) => console.error('Unable to persist preferences', error))
}

function openSettings() {
  populateSettingsForm()
  $('#settings-dialog').showModal()
}

function populateSettingsForm() {
  $('#theme-select').value = state.theme
  $('#content-width').value = state.contentWidth
  $('#ui-font-family').value = state.typography.uiFontFamily
  $('#ui-font-weight').value = String(state.typography.uiFontWeight)
  $('#code-font-family').value = state.typography.codeFontFamily
  $('#code-font-size').value = String(state.typography.codeFontSize)
  $('#code-font-weight').value = String(state.typography.codeFontWeight)
  $('#high-contrast').checked = state.typography.highContrast
  $('#annotation-template').value = state.annotationPromptTemplate
  $('#settings-error').classList.add('hidden')
}

function saveSettings(event) {
  event.preventDefault()
  const template = $('#annotation-template').value.trim()
  if (!template.includes('{{annotations}}')) {
    $('#settings-error').textContent = '批注模板必须包含 {{annotations}}。'
    $('#settings-error').classList.remove('hidden')
    return
  }
  state.theme = $('#theme-select').value === 'dark' ? 'dark' : 'light'
  state.contentWidth = normalizeContentWidth($('#content-width').value)
  state.typography = normalizeTypography({
    uiFontFamily: $('#ui-font-family').value.trim(),
    uiFontWeight: Number($('#ui-font-weight').value),
    codeFontFamily: $('#code-font-family').value.trim(),
    codeFontSize: Number($('#code-font-size').value),
    codeFontWeight: Number($('#code-font-weight').value),
    highContrast: $('#high-contrast').checked,
  })
  state.annotationPromptTemplate = template.slice(0, 32000)
  applyAppearance()
  persistPreferences()
  $('#settings-dialog').close()
}

function resetSettings() {
  state.theme = 'light'
  state.contentWidth = 'comfortable'
  state.typography = { ...typographyDefaults }
  state.annotationPromptTemplate = annotationPromptDefault
  populateSettingsForm()
  applyAppearance()
}

function applyAppearance() {
  const root = document.documentElement
  root.dataset.theme = state.theme
  root.dataset.contentWidth = state.contentWidth
  root.dataset.highContrast = String(state.typography.highContrast)
  root.style.setProperty('--ui-font-family', state.typography.uiFontFamily)
  root.style.setProperty('--ui-font-weight', state.typography.uiFontWeight)
  root.style.setProperty('--code-font-family', state.typography.codeFontFamily)
  root.style.setProperty('--code-font-size', `${state.typography.codeFontSize}px`)
  root.style.setProperty('--code-font-weight', state.typography.codeFontWeight)
}

function openBackendDialog() {
  const info = state.backendInfo || {}
  $('#backend-dialog-content').innerHTML = `
    <div class="detail-row"><span>应用</span><strong>${escapeHtml(info.appName || 'Codex Thread Studio')}</strong></div>
    <div class="detail-row"><span>版本</span><strong>v${escapeHtml(info.appVersion || 'unknown')}</strong></div>
    <div class="detail-row"><span>状态</span><strong>${state.ready ? '已连接' : '未连接'}</strong></div>
    <div class="detail-row"><span>Codex</span><strong>${escapeHtml(info.binary || 'codex')}</strong></div>
    <div class="detail-row"><span>协议</span><strong>${escapeHtml(info.protocol || 'Codex App Server v2')}</strong></div>
    <div class="detail-row"><span>传输</span><strong>${escapeHtml(info.transport || 'stdio JSONL')}</strong></div>`
  $('#backend-dialog').showModal()
}

function setBackendState(kind, label, caption) {
  $('#backend-dot').className = `backend-dot ${kind}`
  $('#backend-label').textContent = label
  $('#backend-caption').textContent = caption
}

function setNativeError(message) {
  $('#native-error').classList.toggle('hidden', !message)
  $('#native-error-message').textContent = message || ''
}

function updateSelectedThreadStatus(message) {
  const thread = selectedThread()
  if (!thread) return
  if (message.method === 'thread/status/changed' && message.params?.threadId === thread.id) thread.status = message.params.status
  renderWorkspace()
}

function renderUsage() {
  const usage = state.model.usage
  if (!usage) return ($('#token-usage').textContent = '')
  const total = usage.totalTokens ?? usage.total_tokens ?? usage.total?.totalTokens
  $('#token-usage').textContent = Number.isFinite(total) ? `${Number(total).toLocaleString()} tokens` : ''
}

function threadTitle(thread) { return thread?.name || thread?.preview || basename(thread?.cwd) || thread?.id || 'Codex 会话' }
function basename(path) { return String(path || '').split(/[\\/]/).filter(Boolean).at(-1) || '' }
function shortId(value) { const text = String(value || ''); return text.length > 12 ? `${text.slice(0, 8)}…` : text }
function threadSourceLabel(source) {
  if (typeof source === 'string') return source
  if (!source || typeof source !== 'object') return ''
  return source.type || source.kind || Object.keys(source)[0] || ''
}
function threadStatus(thread) { return thread?.status?.type || thread?.status || 'notLoaded' }
function statusLabel(status) {
  return ({ active: '运行中', running: '运行中', inProgress: '执行中', idle: '空闲', notLoaded: '未加载', completed: '完成', interrupted: '已停止', failed: '失败', systemError: '异常', declined: '已拒绝' })[status] || status || '未知'
}
function arrayText(value) {
  if (!Array.isArray(value)) return typeof value === 'string' ? value : ''
  return value.map((entry) => typeof entry === 'string' ? entry : entry?.text || valueText(entry)).filter(Boolean).join('\n')
}
function valueText(value) { return typeof value === 'string' ? value : JSON.stringify(value, null, 2) }
function randomId() { return globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}` }
function isTypingTarget(target) { return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement || target?.isContentEditable }
function escapeHtml(value) { return String(value ?? '').replace(/[&<>'"]/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' })[character]) }

function normalizeTypography(value) {
  const weights = [400, 500, 600]
  return {
    uiFontFamily: String(value.uiFontFamily || typographyDefaults.uiFontFamily).slice(0, 512),
    uiFontWeight: weights.includes(Number(value.uiFontWeight)) ? Number(value.uiFontWeight) : 400,
    codeFontFamily: String(value.codeFontFamily || typographyDefaults.codeFontFamily).slice(0, 512),
    codeFontSize: Math.min(20, Math.max(11, Number(value.codeFontSize) || 13)),
    codeFontWeight: weights.includes(Number(value.codeFontWeight)) ? Number(value.codeFontWeight) : 400,
    highContrast: Boolean(value.highContrast),
  }
}

function normalizeContentWidth(value) {
  return ['comfortable', 'wide', 'full'].includes(value) ? value : 'comfortable'
}

function normalizeAnnotationDrafts(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).flatMap(([threadId, drafts]) => {
    if (!threadId || !Array.isArray(drafts)) return []
    const normalized = drafts.slice(0, 32).flatMap((draft) => draft?.quote && draft?.comment ? [{
      id: String(draft.id || randomId()).slice(0, 128),
      quote: String(draft.quote).slice(0, 16000),
      comment: String(draft.comment).slice(0, 16000),
      createdAt: String(draft.createdAt || new Date().toISOString()).slice(0, 128),
      itemId: draft.itemId ? String(draft.itemId).slice(0, 256) : null,
      turnId: draft.turnId ? String(draft.turnId).slice(0, 256) : null,
    }] : [])
    return normalized.length ? [[threadId, normalized]] : []
  }))
}

function normalizeAdditional(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).flatMap(([id, text]) => id && typeof text === 'string' ? [[id, text.slice(0, 32000)]] : []))
}
function normalizeTemplate(value) { return typeof value === 'string' && value.includes('{{annotations}}') ? value.slice(0, 32000) : annotationPromptDefault }

function toast(message, kind = 'info') {
  const element = document.createElement('div')
  element.className = `toast ${kind}`
  element.textContent = message
  $('#toast-region').appendChild(element)
  setTimeout(() => element.remove(), 3200)
}
function showError(error) { console.error(error); toast(error?.message || String(error), 'error') }
