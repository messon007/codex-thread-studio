export function parseUnifiedDiff(source) {
  let oldLine = null
  let newLine = null
  return String(source || '').replace(/\r\n?/gu, '\n').split('\n').map((text) => {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/u.exec(text)
    if (hunk) {
      oldLine = Number(hunk[1])
      newLine = Number(hunk[2])
      return { kind: 'hunk', oldLine: null, newLine: null, text }
    }
    if (oldLine == null || /^(?:diff --git|index |--- |\+\+\+ )/u.test(text)) {
      return { kind: 'meta', oldLine: null, newLine: null, text }
    }
    if (text.startsWith('+')) {
      const row = { kind: 'addition', oldLine: null, newLine, text: text.slice(1) }
      newLine += 1
      return row
    }
    if (text.startsWith('-')) {
      const row = { kind: 'deletion', oldLine, newLine: null, text: text.slice(1) }
      oldLine += 1
      return row
    }
    if (text.startsWith(' ')) {
      const row = { kind: 'context', oldLine, newLine, text: text.slice(1) }
      oldLine += 1
      newLine += 1
      return row
    }
    return { kind: 'note', oldLine: null, newLine: null, text }
  })
}

export function reviewFileStatus(file) {
  if (file?.conflicted) return { label: '!', title: '冲突', tone: 'conflict' }
  if (file?.untracked) return { label: 'U', title: '未跟踪', tone: 'untracked' }
  const code = file?.worktreeStatus?.trim() || file?.indexStatus?.trim() || 'M'
  const labels = { A: '新增', D: '删除', M: '修改', R: '重命名', C: '复制', T: '类型变化' }
  return { label: code, title: labels[code] || '变更', tone: code.toLowerCase() }
}

export function visibleReviewFiles(files, scope = 'all', filter = '') {
  const query = String(filter || '').trim().toLowerCase()
  return (files || []).filter((file) => {
    if (scope === 'staged' && !file.staged) return false
    if (scope === 'unstaged' && !file.unstaged) return false
    return !query || file.path.toLowerCase().includes(query) || String(file.previousPath || '').toLowerCase().includes(query)
  })
}

export function createGitReview({ gatewayFetch, getContext, openFile, translate = (value) => value, notify = () => {} }) {
  const states = new Map()
  const element = (id) => document.getElementById(id)
  let active = false

  function stateForCurrent() {
    const context = getContext?.()
    if (!context?.key || !context?.root) return null
    if (!states.has(context.key)) {
      states.set(context.key, {
        root: context.root, branch: '', files: [], selected: '', scope: 'all', diffScope: 'unstaged',
        filter: '', loading: false, diffLoading: false, error: '', diffError: '', diff: null,
      })
    }
    return states.get(context.key)
  }

  function bind() {
    element('workspace-review-refresh')?.addEventListener('click', () => refresh())
    element('workspace-review-filter')?.addEventListener('input', (event) => {
      const state = stateForCurrent()
      if (!state) return
      state.filter = event.target.value
      render(state)
    })
    element('workspace-review-scopes')?.addEventListener('click', (event) => {
      const button = event.target.closest('[data-review-filter]')
      const state = stateForCurrent()
      if (!button || !state) return
      state.scope = button.dataset.reviewFilter
      chooseVisibleSelection(state)
      render(state)
      loadSelectedDiff(state)
    })
    element('workspace-review-files')?.addEventListener('click', (event) => {
      const row = event.target.closest('[data-review-path]')
      const state = stateForCurrent()
      if (!row || !state) return
      state.selected = row.dataset.reviewPath
      const file = selectedFile(state)
      state.diffScope = file?.unstaged ? 'unstaged' : 'staged'
      render(state)
      loadSelectedDiff(state)
    })
    element('workspace-review-diff-scopes')?.addEventListener('click', (event) => {
      const button = event.target.closest('[data-diff-scope]')
      const state = stateForCurrent()
      if (!button || !state || button.disabled) return
      state.diffScope = button.dataset.diffScope
      render(state)
      loadSelectedDiff(state)
    })
    element('workspace-review-action')?.addEventListener('click', mutateSelected)
    element('workspace-review-open-file')?.addEventListener('click', () => {
      const state = stateForCurrent()
      const file = selectedFile(state)
      if (state && file) openFile?.({ root: state.root, path: file.path }, { returnTool: 'review' })
    })
  }

  async function open() {
    active = true
    const state = stateForCurrent()
    if (!state) return
    element('workspace-review-filter').value = state.filter
    render(state)
    await refresh({ quiet: Boolean(state.files.length) })
  }

  function close() { active = false }

  async function refresh({ quiet = false } = {}) {
    const state = stateForCurrent()
    if (!state || state.loading) return
    state.loading = true
    state.error = ''
    if (!quiet) render(state)
    try {
      const result = await request('/studio/git/status', { root: state.root })
      state.root = result.root || state.root
      state.branch = result.branch || 'HEAD'
      state.files = result.files || []
      chooseVisibleSelection(state)
      render(state)
      await loadSelectedDiff(state)
    } catch (error) {
      state.error = error.message
      state.files = []
      state.diff = null
      render(state)
    } finally {
      state.loading = false
      render(state)
    }
  }

  function chooseVisibleSelection(state) {
    const files = visibleReviewFiles(state.files, state.scope, state.filter)
    if (!files.some((file) => file.path === state.selected)) state.selected = files[0]?.path || ''
    const file = selectedFile(state)
    if (file && !file[state.diffScope]) state.diffScope = file.unstaged ? 'unstaged' : 'staged'
  }

  async function loadSelectedDiff(state = stateForCurrent()) {
    const file = selectedFile(state)
    if (!state || !file) {
      if (state) state.diff = null
      render(state)
      return
    }
    const requestKey = `${file.path}:${state.diffScope}:${Date.now()}`
    state.diffRequest = requestKey
    state.diffLoading = true
    state.diffError = ''
    render(state)
    try {
      const result = await request('/studio/git/diff', { root: state.root, path: file.path, scope: state.diffScope })
      if (state.diffRequest !== requestKey) return
      state.diff = result
    } catch (error) {
      if (state.diffRequest !== requestKey) return
      state.diff = null
      state.diffError = error.message
    } finally {
      if (state.diffRequest === requestKey) {
        state.diffLoading = false
        render(state)
      }
    }
  }

  async function mutateSelected() {
    const state = stateForCurrent()
    const file = selectedFile(state)
    if (!state || !file || state.mutating) return
    const stage = state.diffScope === 'unstaged'
    state.mutating = true
    render(state)
    try {
      const result = await request(stage ? '/studio/git/stage' : '/studio/git/unstage', { root: state.root, paths: [file.path] })
      state.files = result.files || []
      state.branch = result.branch || state.branch
      chooseVisibleSelection(state)
      notify(translate(stage ? '已暂存文件。' : '已取消暂存文件。'))
      render(state)
      await loadSelectedDiff(state)
    } catch (error) {
      notify(error.message, 'error')
    } finally {
      state.mutating = false
      render(state)
    }
  }

  async function request(path, body) {
    const response = await gatewayFetch(path, {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    })
    const result = await response.json().catch(() => null)
    if (!response.ok) throw new Error(result?.error?.message || `HTTP ${response.status}`)
    return result
  }

  function selectedFile(state) { return state?.files.find((file) => file.path === state.selected) || null }

  function render(state = stateForCurrent()) {
    if (!active || !state) return
    const files = visibleReviewFiles(state.files, state.scope, state.filter)
    const counts = {
      all: state.files.length,
      unstaged: state.files.filter((file) => file.unstaged).length,
      staged: state.files.filter((file) => file.staged).length,
    }
    element('workspace-review-branch').textContent = state.branch || 'Git'
    element('workspace-review-count').textContent = translate('{count} 个变更', { count: state.files.length })
    for (const button of element('workspace-review-scopes')?.querySelectorAll('[data-review-filter]') || []) {
      const scope = button.dataset.reviewFilter
      button.classList.toggle('active', state.scope === scope)
      button.setAttribute('aria-selected', String(state.scope === scope))
      const count = button.querySelector('span')
      if (count) count.textContent = counts[scope]
    }
    const list = element('workspace-review-files')
    if (state.loading && !state.files.length) {
      list.innerHTML = `<div class="workspace-review-state">${escapeHtml(translate('正在读取 Git 变更…'))}</div>`
    } else if (state.error) {
      list.innerHTML = `<div class="workspace-review-state error"><strong>${escapeHtml(translate('无法读取 Git 变更'))}</strong><span>${escapeHtml(state.error)}</span></div>`
    } else if (!files.length) {
      list.innerHTML = `<div class="workspace-review-state"><strong>${escapeHtml(translate(state.files.length ? '当前筛选没有变更' : '工作区没有未提交变更'))}</strong><span>${escapeHtml(translate('Review 只显示当前 Git 仓库的文本变更。'))}</span></div>`
    } else {
      list.innerHTML = files.map((file) => {
        const status = reviewFileStatus(file)
        const name = file.path.split('/').at(-1)
        const directory = file.path.slice(0, Math.max(0, file.path.length - name.length)).replace(/\/$/u, '')
        return `<button class="workspace-review-file${file.path === state.selected ? ' selected' : ''}" type="button" data-review-path="${escapeHtml(file.path)}">
          <span class="workspace-review-status ${status.tone}" title="${escapeHtml(translate(status.title))}">${status.label}</span>
          <span class="workspace-review-file-copy"><strong data-no-i18n>${escapeHtml(name)}</strong><small data-no-i18n>${escapeHtml(directory || '.')}</small></span>
          <span class="workspace-review-file-scopes">${file.staged ? '<i title="已暂存">S</i>' : ''}${file.unstaged ? '<i title="未暂存">W</i>' : ''}</span>
        </button>`
      }).join('')
    }
    renderDiff(state)
  }

  function renderDiff(state) {
    const file = selectedFile(state)
    const header = element('workspace-review-diff-header')
    header.classList.toggle('hidden', !file)
    const host = element('workspace-review-diff')
    if (!file) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span>±</span><strong>${escapeHtml(translate('选择文件查看 Diff'))}</strong></div>`
      return
    }
    element('workspace-review-diff-path').textContent = file.path
    for (const button of element('workspace-review-diff-scopes')?.querySelectorAll('[data-diff-scope]') || []) {
      const scope = button.dataset.diffScope
      button.disabled = !file[scope]
      button.classList.toggle('active', state.diffScope === scope)
    }
    const action = element('workspace-review-action')
    action.textContent = state.diffScope === 'staged' ? translate('取消暂存') : translate('暂存文件')
    action.disabled = state.mutating || !file[state.diffScope]
    if (state.diffLoading) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span class="artifact-spinner"></span><strong>${escapeHtml(translate('正在生成 Diff…'))}</strong></div>`
      return
    }
    if (state.diffError) {
      host.innerHTML = `<div class="workspace-review-diff-empty error"><span>!</span><strong>${escapeHtml(state.diffError)}</strong></div>`
      return
    }
    if (state.diff?.binary) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span>◫</span><strong>${escapeHtml(translate('二进制文件无法按行比较'))}</strong></div>`
      return
    }
    const rows = parseUnifiedDiff(state.diff?.content)
    host.innerHTML = `<div class="workspace-review-code" data-extension="${escapeHtml(file.path.split('.').at(-1) || '')}">${rows.map((row) => `<div class="diff-row ${row.kind}"><span class="diff-old">${row.oldLine ?? ''}</span><span class="diff-new">${row.newLine ?? ''}</span><span class="diff-sign">${row.kind === 'addition' ? '+' : row.kind === 'deletion' ? '−' : ''}</span><code>${escapeHtml(row.text)}</code></div>`).join('')}</div>${state.diff?.truncated ? `<div class="workspace-review-truncated">${escapeHtml(translate('Diff 过大，仅显示前 4 MiB。'))}</div>` : ''}`
  }

  return { bind, open, close, refresh, render }
}

function escapeHtml(value) {
  return String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#039;')
}
