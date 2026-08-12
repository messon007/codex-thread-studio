const DEFAULT_WIDTH = 560

export function workspaceRootName(value) {
  const root = String(value || '').replace(/[\\/]+$/u, '')
  return root.split(/[\\/]/u).filter(Boolean).at(-1) || root || 'Workspace'
}

export function safeWorkspaceRelativePath(value) {
  const path = String(value || '').replaceAll('\\', '/').replace(/^\.\//u, '')
  if (!path || path === '.') return ''
  if (path.startsWith('/') || /^[a-z]:\//iu.test(path)) return null
  const parts = path.split('/').filter((part) => part && part !== '.')
  if (parts.some((part) => part === '..')) return null
  return parts.join('/')
}

export function workspaceStateKey(thread, backend = 'codex') {
  const root = String(thread?.cwd || '').trim()
  const session = String(thread?.id || '').trim()
  return root ? `${backend}:${session || root}:${root}` : ''
}

export function createWorkspaceTools({
  gatewayFetch,
  gatewayWebSocket,
  getThread,
  getBackend,
  openFile,
  canOpenFile = () => true,
  closePeerRails,
  translate = (value) => value,
  notify = () => {},
}) {
  const states = new Map()
  let activeTool = null
  let visibleKey = ''
  let resize = null

  const element = (id) => document.getElementById(id)
  const currentThread = () => getThread?.() || null
  const currentKey = () => workspaceStateKey(currentThread(), getBackend?.() || 'codex')
  const currentSessionKey = () => {
    const thread = currentThread()
    return thread?.id ? `${getBackend?.() || 'codex'}:${thread.id}` : ''
  }

  function stateForCurrent() {
    const thread = currentThread()
    const key = currentKey()
    if (!thread?.cwd || !key) return null
    if (!states.has(key)) {
      states.set(key, {
        key,
        root: thread.cwd,
        children: new Map(),
        expanded: new Set(['']),
        loading: new Set(),
        errors: new Map(),
        selected: '',
        filter: '',
        terminalStarted: false,
        terminal: null,
      })
    }
    return states.get(key)
  }

  function bind() {
    element('open-workspace-files')?.addEventListener('click', () => open('files'))
    element('open-workspace-terminal')?.addEventListener('click', () => open('terminal'))
    element('close-workspace-tools')?.addEventListener('click', close)
    element('workspace-files-refresh')?.addEventListener('click', refreshFiles)
    element('workspace-file-filter')?.addEventListener('input', (event) => {
      const state = stateForCurrent()
      if (!state) return
      state.filter = event.target.value.trim().toLowerCase()
      renderFileTree(state)
    })
    element('workspace-file-tree')?.addEventListener('click', handleTreeClick)
    element('start-workspace-terminal')?.addEventListener('click', startTerminal)
    element('clear-workspace-terminal')?.addEventListener('click', clearTerminal)
    element('stop-workspace-terminal')?.addEventListener('click', stopTerminal)
    const resizer = element('workspace-tools-resizer')
    resizer?.addEventListener('pointerdown', beginResize)
    resizer?.addEventListener('pointermove', continueResize)
    resizer?.addEventListener('pointerup', finishResize)
    resizer?.addEventListener('pointercancel', finishResize)
    resizer?.addEventListener('dblclick', resetWidth)
    resizer?.addEventListener('keydown', resizeWithKeyboard)
  }

  function sync(thread = currentThread()) {
    const available = Boolean(thread?.cwd)
    for (const id of ['open-workspace-files', 'open-workspace-terminal']) {
      const button = element(id)
      if (button) button.disabled = !available
    }
    if (!available) {
      close()
      return
    }
    if (isOpen() && visibleKey && visibleKey !== currentSessionKey()) {
      close()
      return
    }
    const state = stateForCurrent()
    element('workspace-tools-path').textContent = state.root
    element('workspace-root-name').textContent = workspaceRootName(state.root)
    element('workspace-terminal-cwd').textContent = state.root
    element('workspace-file-filter').value = state.filter
    if (activeTool === 'files') ensureDirectory(state, '').then(() => renderFileTree(state))
    if (activeTool === 'terminal') renderTerminal(state)
    syncButtons()
  }

  async function open(tool) {
    if (!currentThread()?.cwd) {
      notify(translate('当前会话没有项目目录。'), 'error')
      return
    }
    closePeerRails?.()
    activeTool = tool
    visibleKey = currentSessionKey()
    element('workspace-tools-rail').classList.remove('hidden')
    setTool(tool)
    sync()
    if (tool === 'files') await ensureDirectory(stateForCurrent(), '')
    render()
    if (tool === 'terminal' && stateForCurrent()?.terminalStarted) {
      setTimeout(() => fitTerminal(stateForCurrent(), { focus: true }), 30)
    }
  }

  function close() {
    activeTool = null
    visibleKey = ''
    element('workspace-tools-rail')?.classList.add('hidden')
    syncButtons()
  }

  function isOpen() {
    return Boolean(activeTool) && !element('workspace-tools-rail')?.classList.contains('hidden')
  }

  function setTool(tool) {
    if (!['files', 'terminal'].includes(tool)) return
    activeTool = tool
    element('workspace-files-pane').classList.toggle('hidden', tool !== 'files')
    element('workspace-terminal-pane').classList.toggle('hidden', tool !== 'terminal')
    syncButtons()
    render()
  }

  function syncButtons() {
    element('open-workspace-files')?.setAttribute('aria-pressed', String(activeTool === 'files'))
    element('open-workspace-terminal')?.setAttribute('aria-pressed', String(activeTool === 'terminal'))
  }

  function render() {
    const state = stateForCurrent()
    if (!state || !isOpen()) return
    element('workspace-tools-title').textContent = activeTool === 'terminal' ? 'Terminal' : 'Files'
    if (activeTool === 'files') renderFileTree(state)
    else renderTerminal(state)
  }

  async function ensureDirectory(state, relativePath) {
    const path = safeWorkspaceRelativePath(relativePath)
    if (!state || path == null || state.children.has(path) || state.loading.has(path)) return
    state.loading.add(path)
    state.errors.delete(path)
    renderFileTree(state)
    try {
      const response = await gatewayFetch('/studio/workspace/list', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ root: state.root, path }),
      })
      const result = await response.json().catch(() => null)
      if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
      const entries = (result?.entries || []).filter((entry) => safeWorkspaceRelativePath(entry.path) != null)
      state.children.set(path, entries)
      if (result?.truncated) state.errors.set(path, translate('目录内容过多，仅显示前 500 项。'))
    } catch (error) {
      state.errors.set(path, error.message)
    } finally {
      state.loading.delete(path)
      if (state === stateForCurrent()) renderFileTree(state)
    }
  }

  function visibleRows(state, parent = '', depth = 0, output = []) {
    const entries = state.children.get(parent) || []
    for (const entry of entries) {
      const matches = !state.filter || entry.name.toLowerCase().includes(state.filter) || entry.path.toLowerCase().includes(state.filter)
      if (matches || entry.kind === 'directory') output.push({ ...entry, depth, matches })
      if (entry.kind === 'directory' && state.expanded.has(entry.path)) visibleRows(state, entry.path, depth + 1, output)
    }
    return output
  }

  function renderFileTree(state) {
    const tree = element('workspace-file-tree')
    if (!tree) return
    if (state.loading.has('') && !state.children.has('')) {
      tree.innerHTML = `<div class="workspace-tree-state">${escapeHtml(translate('正在读取当前会话目录…'))}</div>`
      return
    }
    if (state.errors.has('') && !state.children.has('')) {
      tree.innerHTML = `<div class="workspace-tree-state">${escapeHtml(state.errors.get(''))}</div>`
      return
    }
    const rows = visibleRows(state).filter((row) => !state.filter || row.matches || row.kind === 'directory')
    tree.innerHTML = rows.map((row) => {
      const directory = row.kind === 'directory'
      const openable = directory || canOpenFile({ path: row.path })
      const expanded = directory && state.expanded.has(row.path)
      const loading = directory && state.loading.has(row.path)
      return `<button class="workspace-tree-row ${directory ? 'directory' : 'file'}${state.selected === row.path ? ' selected' : ''}${openable ? '' : ' unavailable'}" type="button" role="treeitem" data-path="${escapeHtml(row.path)}" data-kind="${row.kind}" aria-disabled="${String(!openable)}" aria-expanded="${directory ? String(expanded) : ''}" style="--tree-depth:${row.depth}">
        <span class="tree-twisty">${directory ? (loading ? '·' : expanded ? '▾' : '▸') : ''}</span>
        <span class="tree-icon">${directory ? '▱' : fileIcon(row.name)}</span>
        <span class="tree-name" data-no-i18n>${escapeHtml(row.name)}</span>
        ${directory || row.size == null ? '' : `<span class="tree-size">${formatSize(row.size)}</span>`}
      </button>`
    }).join('') || `<div class="workspace-tree-state">${escapeHtml(translate(state.filter ? '没有匹配的文件。' : '目录为空。'))}</div>`
  }

  async function handleTreeClick(event) {
    const row = event.target.closest('.workspace-tree-row')
    const state = stateForCurrent()
    if (!row || !state) return
    const path = safeWorkspaceRelativePath(row.dataset.path)
    if (path == null) return
    if (row.getAttribute('aria-disabled') === 'true') {
      notify(translate('此文件类型暂不支持预览。'))
      return
    }
    state.selected = path
    if (row.dataset.kind === 'directory') {
      if (state.expanded.has(path)) state.expanded.delete(path)
      else {
        state.expanded.add(path)
        await ensureDirectory(state, path)
      }
      renderFileTree(state)
      return
    }
    renderFileTree(state)
    close()
    await openFile?.({ root: state.root, path })
  }

  function refreshFiles() {
    const state = stateForCurrent()
    if (!state) return
    state.children.clear()
    state.errors.clear()
    ensureDirectory(state, '').then(() => renderFileTree(state))
  }

  function clearTerminal() {
    const state = stateForCurrent()
    state?.terminal?.view?.clear()
  }

  async function startTerminal() {
    const state = stateForCurrent()
    if (!state || state.terminalStarted) return
    state.terminalStarted = true
    renderTerminal(state)
    let Terminal
    let FitAddon
    try {
      ;({ Terminal, FitAddon } = await import('./vendor/workspace-terminal.mjs'))
    } catch (error) {
      state.terminalStarted = false
      renderTerminal(state)
      notify(`${translate('无法加载终端组件')}: ${error.message}`, 'error')
      return
    }
    if (state !== stateForCurrent() || !state.terminalStarted) {
      state.terminalStarted = false
      return
    }
    const host = document.createElement('div')
    host.className = 'workspace-terminal-xterm'
    const view = new Terminal({
      allowProposedApi: false,
      convertEol: false,
      cursorBlink: true,
      cursorStyle: 'bar',
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--code-font-family').trim() || 'monospace',
      fontSize: 12,
      fontWeight: '500',
      letterSpacing: 0,
      lineHeight: 1.2,
      scrollback: 10_000,
      theme: {
        background: '#111719',
        foreground: '#dbe8e7',
        cursor: '#6cdbb8',
        selectionBackground: '#315a52',
        black: '#111719', red: '#ec7c86', green: '#6dd7aa', yellow: '#e2c275',
        blue: '#77a9e8', magenta: '#c994dc', cyan: '#69cbd1', white: '#dbe8e7',
        brightBlack: '#718485', brightRed: '#ff9aa2', brightGreen: '#8be7c0', brightYellow: '#f4d58c',
        brightBlue: '#94bdf0', brightMagenta: '#dfabea', brightCyan: '#8de0e2', brightWhite: '#f3f8f7',
      },
    })
    const fit = new FitAddon()
    view.loadAddon(fit)
    state.terminal = { host, view, fit, socket: null, resizeObserver: null, ready: false }
    renderTerminal(state)
    const terminal = state.terminal
    const resizeObserver = new ResizeObserver(() => fitTerminal(state))
    terminal.resizeObserver = resizeObserver
    resizeObserver.observe(host)
    view.onData((data) => {
      if (terminal.socket?.readyState === WebSocket.OPEN && terminal.ready) {
        terminal.socket.send(new TextEncoder().encode(data))
      }
    })
    view.onResize(({ cols, rows }) => {
      if (terminal.socket?.readyState === WebSocket.OPEN && terminal.ready) {
        terminal.socket.send(JSON.stringify({ type: 'resize', cols, rows }))
      }
    })
    setTimeout(() => {
      fitTerminal(state, { focus: true })
      connectTerminal(state)
    }, 30)
  }

  function stopTerminal() {
    const state = stateForCurrent()
    if (!state?.terminalStarted) return
    const terminal = state.terminal
    if (terminal?.socket?.readyState === WebSocket.OPEN) {
      terminal.socket.send(JSON.stringify({ type: 'stop' }))
      terminal.socket.close(1000, 'terminal stopped')
    }
    terminal?.resizeObserver?.disconnect()
    terminal?.view?.dispose()
    state.terminalStarted = false
    state.terminal = null
    renderTerminal(state)
  }

  function renderTerminal(state) {
    element('workspace-terminal-disconnected').classList.toggle('hidden', state.terminalStarted)
    element('workspace-terminal-session').classList.toggle('hidden', !state.terminalStarted)
    if (!state.terminalStarted) return
    const screen = element('workspace-terminal-screen')
    if (!screen || !state.terminal?.host) return
    if (state.terminal.host.parentElement !== screen) screen.replaceChildren(state.terminal.host)
    if (!state.terminal.view.element) state.terminal.view.open(state.terminal.host)
    setTimeout(() => fitTerminal(state), 0)
  }

  function connectTerminal(state) {
    const terminal = state?.terminal
    if (!terminal || terminal.socket) return
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/terminal`)
    socket.binaryType = 'arraybuffer'
    terminal.socket = socket
    socket.addEventListener('open', () => {
      if (state.terminal !== terminal) return socket.close()
      fitTerminal(state)
      socket.send(JSON.stringify({
        type: 'start', root: state.root,
        cols: terminal.view.cols || 80,
        rows: terminal.view.rows || 24,
      }))
    })
    socket.addEventListener('message', (event) => {
      if (state.terminal !== terminal) return
      if (event.data instanceof ArrayBuffer) {
        terminal.view.write(new Uint8Array(event.data))
        return
      }
      let message
      try { message = JSON.parse(String(event.data || '')) } catch { return }
      if (message.type === 'ready') {
        terminal.ready = true
        element('workspace-terminal-shell').textContent = message.shell || 'shell'
        terminal.view.focus()
      } else if (message.type === 'error') {
        terminal.view.writeln(`\r\n\x1b[31m${message.message || translate('终端连接失败')}\x1b[0m`)
      } else if (message.type === 'exit') {
        terminal.ready = false
        terminal.view.writeln(`\r\n\x1b[90m[${translate('终端已退出')} ${message.code ?? ''}]\x1b[0m`)
      }
    })
    socket.addEventListener('close', () => {
      if (state.terminal !== terminal) return
      terminal.ready = false
    })
    socket.addEventListener('error', () => {
      if (state.terminal !== terminal) return
      terminal.view.writeln(`\r\n\x1b[31m${translate('无法连接终端')}\x1b[0m`)
    })
  }

  function fitTerminal(state, { focus = false } = {}) {
    const terminal = state?.terminal
    if (!terminal?.view?.element || !terminal.host.isConnected || terminal.host.clientWidth < 10 || terminal.host.clientHeight < 10) return
    try { terminal.fit.fit() } catch {}
    if (focus) terminal.view.focus()
  }

  function beginResize(event) {
    if (event.button !== 0) return
    const rail = element('workspace-tools-rail')
    resize = { pointerId: event.pointerId, startX: event.clientX, startWidth: rail.getBoundingClientRect().width }
    event.currentTarget.setPointerCapture(event.pointerId)
    document.body.classList.add('resizing-workspace-tools')
  }

  function continueResize(event) {
    if (!resize || resize.pointerId !== event.pointerId) return
    setWidth(resize.startWidth + resize.startX - event.clientX)
  }

  function finishResize(event) {
    if (!resize || resize.pointerId !== event.pointerId) return
    resize = null
    document.body.classList.remove('resizing-workspace-tools')
  }

  function resizeWithKeyboard(event) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    const current = element('workspace-tools-rail').getBoundingClientRect().width
    if (event.key === 'Home') setWidth(420)
    else if (event.key === 'End') setWidth(widthBounds().max)
    else setWidth(current + (event.key === 'ArrowLeft' ? 28 : -28))
  }

  function widthBounds() {
    const sidebar = document.getElementById('sidebar')?.getBoundingClientRect().width || 0
    return { min: 420, max: Math.max(420, Math.min(780, window.innerWidth - sidebar - 570)) }
  }

  function setWidth(value) {
    const { min, max } = widthBounds()
    document.documentElement.style.setProperty('--workspace-tools-width', `${Math.round(Math.max(min, Math.min(max, value)))}px`)
    if (activeTool === 'terminal') setTimeout(() => fitTerminal(stateForCurrent()), 0)
  }

  function resetWidth() { setWidth(DEFAULT_WIDTH) }

  return { bind, sync, open, close, isOpen }
}

function fileIcon(name) {
  if (/\.md$/iu.test(name)) return 'M'
  if (/\.(js|mjs|ts|tsx)$/iu.test(name)) return 'J'
  if (/\.rs$/iu.test(name)) return 'R'
  if (/\.(json|toml|ya?ml)$/iu.test(name)) return '◇'
  return '·'
}

function formatSize(value) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} K`
  return `${(value / 1024 / 1024).toFixed(1)} M`
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;')
}
