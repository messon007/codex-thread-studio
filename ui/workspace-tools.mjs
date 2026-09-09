import { createGitReview } from './git-review.mjs'

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
  let developerThread = null
  let developerBackend = null
  let sourceOwner = ''
  let fileWatch = null
  let fileWatchRetry = null
  let closeLocationMenu = () => {}

  function stopFileWatch() {
    clearTimeout(fileWatchRetry)
    fileWatchRetry = null
    const previous = fileWatch
    fileWatch = null
    previous?.socket.close()
  }

  function watchedPaths(state) {
    return ['', ...visibleRows(state).filter((row) => row.kind === 'directory' && state.expanded.has(row.path)).map((row) => row.path)].slice(0, 128)
  }

  function syncFileWatch(state) {
    if (!gatewayWebSocket || !state || state !== stateForCurrent() || activeTool !== 'files' || !isOpen()) return
    if (fileWatch && fileWatch.state !== state) stopFileWatch()
    if (!fileWatch) {
      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
      const socket = gatewayWebSocket(`${protocol}//${location.host}/ws/workspace-files`)
      const watch = { socket, state, subscription: '' }
      fileWatch = watch
      socket.addEventListener('open', () => { if (fileWatch === watch) syncFileWatch(state) })
      socket.addEventListener('message', (event) => {
        if (fileWatch !== watch) return
        let message
        try { message = JSON.parse(event.data) } catch { return }
        if (message.type === 'ready' || message.type === 'changed') {
          const visible = new Set(watchedPaths(state))
          for (const path of Array.isArray(message.paths) ? message.paths : []) {
            if (visible.has(path)) void ensureDirectory(state, path, true)
          }
        } else if (message.type === 'error') {
          console.warn('Files automatic refresh:', message.message)
        }
      })
      socket.addEventListener('close', () => {
        if (fileWatch !== watch) return
        fileWatch = null
        fileWatchRetry = setTimeout(() => syncFileWatch(state), 5000)
      })
    }
    const subscription = JSON.stringify({ root: state.root, paths: watchedPaths(state) })
    if (fileWatch.socket.readyState === 1 && subscription !== fileWatch.subscription) {
      fileWatch.subscription = subscription
      fileWatch.socket.send(subscription)
    }
  }

  const element = (id) => document.getElementById(id)
  const currentThread = () => developerThread || getThread?.() || null
  const currentBackend = () => developerBackend || getBackend?.() || 'codex'
  const ownerKey = () => workspaceStateKey(getThread?.(), getBackend?.() || 'codex')
  const currentKey = () => workspaceStateKey(currentThread(), currentBackend())
  const currentSessionKey = () => {
    const thread = currentThread()
    return thread?.id ? `${currentBackend()}:${thread.id}` : ''
  }
  const gitReview = createGitReview({
    gatewayFetch,
    getContext: () => {
      const state = stateForCurrent()
      return state ? { key: state.key, root: state.root } : null
    },
    openFile: (file, context) => openFile?.({ ...file, sourceSessionKey: sourceOwner ? currentSessionKey() : '' }, context),
    translate,
    notify,
  })

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
        refreshPending: new Set(),
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
    element('open-workspace-review')?.addEventListener('click', () => open('review'))
    element('close-workspace-tools')?.addEventListener('click', close)
    element('workspace-files-refresh')?.addEventListener('click', refreshFiles)
    const locationButton = element('workspace-open-location')
    if (locationButton) {
      locationButton.disabled = Boolean(window.__CODEX_THREAD_STUDIO_GATEWAY__?.remote)
      if (locationButton.disabled) locationButton.title = translate('System file manager is unavailable in remote browser mode.')
      locationButton.addEventListener('click', () => openItemLocation())
    }
    element('workspace-file-filter')?.addEventListener('input', (event) => {
      const state = stateForCurrent()
      if (!state) return
      state.filter = event.target.value.trim().toLowerCase()
      renderFileTree(state)
    })
    element('workspace-file-tree')?.addEventListener('click', handleTreeClick)
    element('workspace-file-tree')?.addEventListener('contextmenu', event => {
      const row = event.target.closest('.workspace-tree-row')
      if (!row || window.__CODEX_THREAD_STUDIO_GATEWAY__?.remote) return
      event.preventDefault()
      closeLocationMenu()
      const state = stateForCurrent()
      if (!state) return
      state.selected = row.dataset.path
      renderFileTree(state)
      const menu = document.createElement('div')
      menu.className = 'workspace-location-menu'
      menu.setAttribute('role', 'menu')
      const button = document.createElement('button')
      button.type = 'button'
      button.setAttribute('role', 'menuitem')
      button.textContent = translate('Open Item Location')
      menu.append(button)
      document.body.append(menu)
      menu.style.left = `${Math.max(0, Math.min(event.clientX, innerWidth - menu.offsetWidth))}px`
      menu.style.top = `${Math.max(0, Math.min(event.clientY, innerHeight - menu.offsetHeight))}px`
      const closeMenu = () => { menu.remove(); document.removeEventListener('pointerdown', outside, true); document.removeEventListener('keydown', keydown, true) }
      closeLocationMenu = closeMenu
      const outside = event => { if (!menu.contains(event.target)) closeMenu() }
      const keydown = event => { if (event.key === 'Escape') { event.preventDefault(); closeMenu() } }
      document.addEventListener('pointerdown', outside, true)
      document.addEventListener('keydown', keydown, true)
      button.addEventListener('click', () => { closeMenu(); void openItemLocation(state) })
      button.focus()
    })
    element('start-workspace-terminal')?.addEventListener('click', startTerminal)
    element('clear-workspace-terminal')?.addEventListener('click', clearTerminal)
    element('stop-workspace-terminal')?.addEventListener('click', stopTerminal)
    gitReview.bind()
  }

  function sync(thread = currentThread()) {
    if (sourceOwner && sourceOwner !== ownerKey()) { close(); return }
    const available = Boolean(thread?.cwd)
    for (const id of ['open-workspace-files', 'open-workspace-terminal', 'open-workspace-review']) {
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
    element('workspace-tools-path').textContent = sourceOwner ? `${currentThread()?.name || currentThread()?.id} · ${state.root}` : state.root
    element('workspace-root-name').textContent = workspaceRootName(state.root)
    element('workspace-terminal-cwd').textContent = state.root
    element('workspace-file-filter').value = state.filter
    if (activeTool === 'terminal') renderTerminal(state)
    if (activeTool === 'review') gitReview.render()
    syncButtons()
  }

  async function open(tool) {
    if (!currentThread()?.cwd) {
      notify(translate('The current session has no project directory.'), 'error')
      return
    }
    closePeerRails?.()
    activeTool = tool
    visibleKey = currentSessionKey()
    const openingKey = visibleKey
    element('workspace-tools-rail').classList.remove('hidden')
    setTool(tool)
    sync()
    if (tool === 'files') await ensureDirectory(stateForCurrent(), '')
    if (tool === 'review') await gitReview.open()
    if (visibleKey !== openingKey || activeTool !== tool) return
    render()
    if (tool === 'terminal' && stateForCurrent()?.terminalStarted) {
      const terminalState = stateForCurrent()
      setTimeout(() => {
        if (visibleKey === openingKey && activeTool === tool) fitTerminal(terminalState, { focus: true })
      }, 30)
    }
  }

  function close() {
    closeLocationMenu()
    stopFileWatch()
    gitReview.close()
    activeTool = null
    visibleKey = ''
    element('workspace-tools-rail')?.classList.add('hidden')
    syncButtons()
    developerThread = null
    developerBackend = null
    sourceOwner = ''
  }

  function isOpen() {
    return Boolean(activeTool) && !element('workspace-tools-rail')?.classList.contains('hidden')
  }

  function setTool(tool) {
    if (!['files', 'terminal', 'review'].includes(tool)) return
    if (tool !== 'review') gitReview.close()
    if (tool !== 'files') stopFileWatch()
    activeTool = tool
    element('workspace-files-pane').classList.toggle('hidden', tool !== 'files')
    element('workspace-terminal-pane').classList.toggle('hidden', tool !== 'terminal')
    element('workspace-review-pane').classList.toggle('hidden', tool !== 'review')
    syncButtons()
    render()
  }

  function syncButtons() {
    element('open-workspace-files')?.setAttribute('aria-pressed', String(activeTool === 'files'))
    element('open-workspace-terminal')?.setAttribute('aria-pressed', String(activeTool === 'terminal'))
    element('open-workspace-review')?.setAttribute('aria-pressed', String(activeTool === 'review'))
  }

  function render() {
    const state = stateForCurrent()
    if (!state || !isOpen()) return
    element('workspace-tools-title').textContent = activeTool === 'terminal' ? 'Terminal' : activeTool === 'review' ? 'Review' : 'Files'
    if (activeTool === 'files') renderFileTree(state)
    else if (activeTool === 'terminal') renderTerminal(state)
    else gitReview.render()
  }

  async function ensureDirectory(state, relativePath, refresh = false) {
    const path = safeWorkspaceRelativePath(relativePath)
    if (!state || path == null) return
    if (state.loading.has(path)) {
      if (refresh) state.refreshPending.add(path)
      return
    }
    if (!refresh && state.children.has(path)) { syncFileWatch(state); return }
    state.loading.add(path)
    state.errors.delete(path)
    if (state === stateForCurrent() && activeTool === 'files' && !refresh) renderFileTree(state)
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
      if (result?.truncated) state.errors.set(path, translate('This directory is large; only the first 500 entries are shown.'))
    } catch (error) {
      state.errors.set(path, error.message)
    } finally {
      state.loading.delete(path)
      if (state === stateForCurrent() && activeTool === 'files' && isOpen()) {
        renderFileTree(state)
        syncFileWatch(state)
        if (state.refreshPending.delete(path)) void ensureDirectory(state, path, true)
      } else state.refreshPending.delete(path)
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
      tree.innerHTML = `<div class="workspace-tree-state">${escapeHtml(translate('Reading the current session directory…'))}</div>`
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
    }).join('') || `<div class="workspace-tree-state">${escapeHtml(translate(state.filter ? 'No matching files.' : 'The directory is empty.'))}</div>`
  }

  async function handleTreeClick(event) {
    const row = event.target.closest('.workspace-tree-row')
    const state = stateForCurrent()
    if (!row || !state) return
    const path = safeWorkspaceRelativePath(row.dataset.path)
    if (path == null) return
    if (row.getAttribute('aria-disabled') === 'true') {
      notify(translate('This file type cannot be previewed yet.'))
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
      syncFileWatch(state)
      return
    }
    renderFileTree(state)
    const sourceSessionKey = sourceOwner ? currentSessionKey() : ''
    close()
    await openFile?.({ root: state.root, path, sourceSessionKey }, { returnTool: 'files' })
  }

  function refreshFiles() {
    const state = stateForCurrent()
    if (!state) return
    for (const path of watchedPaths(state)) void ensureDirectory(state, path, true)
  }

  async function openItemLocation(state = stateForCurrent()) {
    if (!state) return
    try {
      const response = await gatewayFetch('/studio/workspace/open-location', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ root: state.root, path: state.selected || '' }) })
      if (!response.ok) throw new Error((await response.json()).error || 'Unable to open item location')
    } catch (error) { notify(error.message, 'error') }
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
      notify(`${translate('Unable to load terminal component')}: ${error.message}`, 'error')
      return
    }
    if (state !== stateForCurrent() || !state.terminalStarted) {
      state.terminalStarted = false
      return
    }
    const host = document.createElement('div')
    host.className = 'workspace-terminal-xterm'
    const rootStyle = getComputedStyle(document.documentElement)
    const codeFontSize = Number.parseFloat(rootStyle.getPropertyValue('--code-font-size')) || 14
    const view = new Terminal({
      allowProposedApi: false,
      convertEol: false,
      cursorBlink: true,
      cursorStyle: 'bar',
      fontFamily: rootStyle.getPropertyValue('--code-font-family').trim() || 'monospace',
      fontSize: codeFontSize,
      fontWeight: rootStyle.getPropertyValue('--code-font-weight').trim() || '500',
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
        terminal.view.writeln(`\r\n\x1b[31m${message.message || translate('Terminal connection failed')}\x1b[0m`)
      } else if (message.type === 'exit') {
        terminal.ready = false
        terminal.view.writeln(`\r\n\x1b[90m[${translate('Terminal exited')} ${message.code ?? ''}]\x1b[0m`)
      }
    })
    socket.addEventListener('close', () => {
      if (state.terminal !== terminal) return
      terminal.ready = false
    })
    socket.addEventListener('error', () => {
      if (state.terminal !== terminal) return
      terminal.view.writeln(`\r\n\x1b[31m${translate('Unable to connect to terminal')}\x1b[0m`)
    })
  }

  function fitTerminal(state, { focus = false } = {}) {
    const terminal = state?.terminal
    if (!terminal?.view?.element || !terminal.host.isConnected || terminal.host.clientWidth < 10 || terminal.host.clientHeight < 10) return
    try { terminal.fit.fit() } catch {}
    if (focus) terminal.view.focus()
  }

  function refreshTypography() {
    const rootStyle = getComputedStyle(document.documentElement)
    const fontFamily = rootStyle.getPropertyValue('--code-font-family').trim() || 'monospace'
    const fontSize = Number.parseFloat(rootStyle.getPropertyValue('--code-font-size')) || 14
    const fontWeight = rootStyle.getPropertyValue('--code-font-weight').trim() || '500'
    for (const state of states.values()) {
      if (!state.terminal?.view) continue
      state.terminal.view.options.fontFamily = fontFamily
      state.terminal.view.options.fontSize = fontSize
      state.terminal.view.options.fontWeight = fontWeight
      fitTerminal(state)
    }
  }

  async function openForDebug(root, tool = 'files') {
    developerThread = { id: 'developer-preview', cwd: String(root || '') }
    await open(tool)
  }

  async function openForSession(thread, backend, tool = 'files') {
    close()
    developerThread = thread
    developerBackend = backend
    sourceOwner = ownerKey()
    await open(tool)
    if (developerThread !== thread || developerBackend !== backend || !isOpen()) return
    element('workspace-tools-path').textContent = `${thread.name || thread.title || thread.id} · ${thread.cwd}`
    element('workspace-terminal-cwd').textContent = `${thread.name || thread.title || thread.id} · ${thread.cwd}`
  }

  async function reveal(path) {
    const relativePath = safeWorkspaceRelativePath(path)
    if (relativePath == null) return false
    await open('files')
    const state = stateForCurrent()
    if (!state) return false
    state.filter = ''
    element('workspace-file-filter').value = ''
    const segments = relativePath.split('/').filter(Boolean)
    let parent = ''
    await ensureDirectory(state, '')
    for (const segment of segments.slice(0, -1)) {
      parent = parent ? `${parent}/${segment}` : segment
      state.expanded.add(parent)
      await ensureDirectory(state, parent)
    }
    state.selected = relativePath
    renderFileTree(state)
    requestAnimationFrame(() => element('workspace-file-tree')?.querySelector(`[data-path="${CSS.escape(relativePath)}"]`)?.scrollIntoView({ block: 'nearest' }))
    return true
  }

  function snapshot() {
    if (!isOpen()) return null
    return { tool: activeTool, source: sourceOwner ? { backend: currentBackend(), id: currentThread()?.id } : null,
      scroll: ['workspace-file-tree', 'workspace-review-files', 'workspace-review-diff'].map(id => ({ id, top: element(id)?.scrollTop || 0, left: element(id)?.scrollLeft || 0 })) }
  }
  return { bind, sync, open, openForDebug, openForSession, reveal, close, isOpen, snapshot, resize: () => fitTerminal(stateForCurrent()), refreshTypography }
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
