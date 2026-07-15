import {
  applyCodexNotification,
  createCodexViewModel,
  hydrateCodexThread,
  resolveCodexApproval,
  textFromUserContent,
} from './codex-native.mjs'
import { marked } from './vendor/marked.esm.js'
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
}

let preferencesReady = false
let preferencesWriteChain = Promise.resolve()
let annotationPersistTimer = null

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
  $('#composer-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault()
      $('#composer-form').requestSubmit()
    }
  })
  $('#interrupt-turn').addEventListener('click', interruptTurn)
  $('#transcript').addEventListener('mouseup', captureTranscriptSelection)
  $('#transcript').addEventListener('click', handleTranscriptClick)
  $('#comment-selection').addEventListener('mousedown', (event) => event.preventDefault())
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
    renderTranscript(message.method?.includes('/delta') || message.method === 'item/started')
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
  state.selectedId = id
  state.model = createCodexViewModel()
  state.model.threadId = id
  persistPreferences()
  renderThreadList()
  renderWorkspace()
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

async function refreshSelectedThread() {
  if (!state.selectedId) return
  try {
    const result = await rpc('thread/read', { threadId: state.selectedId, includeTurns: true })
    hydrateCodexThread(state.model, result.thread)
    mergeThreadMetadata(result.thread)
    renderWorkspace()
    renderTranscript(false)
    toast('会话已刷新')
  } catch (error) { showError(error) }
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

function renderTranscript(followOutput) {
  if (!state.selectedId) return
  const container = $('#transcript')
  const nearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 100
  const turns = state.model.turns || []
  container.innerHTML = turns.map((turn, index) => renderTurn(turn, index)).join('') + renderApprovals()
  bindApprovalButtons()
  if (followOutput && nearBottom) requestAnimationFrame(() => { container.scrollTop = container.scrollHeight })
  renderUsage()
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

function renderComposerState() {
  const active = Boolean(state.model.activeTurnId)
  $('#interrupt-turn').classList.toggle('hidden', !active)
  $('#stop-thread').classList.toggle('hidden', !active)
  $('#archive-thread').disabled = active
  $('#delete-thread').disabled = active
  $('#send-message').textContent = active ? '追加意见' : '发送'
  $('#composer-hint').textContent = active ? '将通过 turn/steer 加入当前 Turn' : '将通过 turn/start 开始新 Turn'
  $('#send-message').disabled = !state.ready || !state.selectedId
  $('#thread-status').textContent = statusLabel(state.model.status)
  $('#thread-status').className = `status-badge ${state.model.status}`
}

async function sendComposer(event) {
  event.preventDefault()
  const input = $('#composer-input')
  const text = input.value.trim()
  if (!text || !state.selectedId) return
  const button = $('#send-message')
  button.disabled = true
  try {
    if (state.model.activeTurnId) {
      await rpc('turn/steer', {
        threadId: state.selectedId,
        expectedTurnId: state.model.activeTurnId,
        clientUserMessageId: randomId(),
        input: [{ type: 'text', text }],
      })
      toast('意见已加入当前 Turn')
    } else {
      const result = await rpc('turn/start', {
        threadId: state.selectedId,
        clientUserMessageId: randomId(),
        input: [{ type: 'text', text }],
      })
      if (result?.turn) {
        applyCodexNotification(state.model, { method: 'turn/started', params: { turn: result.turn } })
        renderTranscript(true)
      }
    }
    input.value = ''
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
