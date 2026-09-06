import { parseUnifiedDiff, reviewFileStatus, visibleReviewFiles, createPendingGitReads } from './git-review-data.mjs'
export { parseUnifiedDiff, reviewFileStatus, visibleReviewFiles, createPendingGitReads } from './git-review-data.mjs'

export function createGitReview({ gatewayFetch, getContext, openFile, translate = (value) => value, notify = () => {} }) {
  const states = new Map()
  const pendingDiffs = createPendingGitReads()
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
      state.diffRevision = (state.diffRevision || 0) + 1
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
      if (state) {
        state.diffRequest = null
        state.diffLoading = false
        state.diff = null
      }
      render(state)
      return
    }
    const requestKey = Symbol('diff request')
    const body = { root: state.root, path: file.path, scope: state.diffScope }
    const readKey = JSON.stringify([body, state.diffRevision || 0])
    state.diffRequest = requestKey
    state.diffLoading = true
    state.diffError = ''
    render(state)
    try {
      const result = await pendingDiffs(readKey, () => request('/studio/git/diff', body))
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
      state.diffRevision = (state.diffRevision || 0) + 1
      state.branch = result.branch || state.branch
      chooseVisibleSelection(state)
      notify(translate(stage ? 'File staged.' : 'File unstaged.'))
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
    element('workspace-review-count').textContent = translate('{count} changes', { count: state.files.length })
    for (const button of element('workspace-review-scopes')?.querySelectorAll('[data-review-filter]') || []) {
      const scope = button.dataset.reviewFilter
      button.classList.toggle('active', state.scope === scope)
      button.setAttribute('aria-selected', String(state.scope === scope))
      const count = button.querySelector('span')
      if (count) count.textContent = counts[scope]
    }
    const list = element('workspace-review-files')
    if (state.loading && !state.files.length) {
      list.innerHTML = `<div class="workspace-review-state">${escapeHtml(translate('Reading Git changes…'))}</div>`
    } else if (state.error) {
      list.innerHTML = `<div class="workspace-review-state error"><strong>${escapeHtml(translate('Unable to read Git changes'))}</strong><span>${escapeHtml(state.error)}</span></div>`
    } else if (!files.length) {
      list.innerHTML = `<div class="workspace-review-state"><strong>${escapeHtml(translate(state.files.length ? 'No changes match this filter' : 'The working tree has no uncommitted changes'))}</strong><span>${escapeHtml(translate('Review shows text changes in the current Git repository.'))}</span></div>`
    } else {
      list.innerHTML = files.map((file) => {
        const status = reviewFileStatus(file)
        const name = file.path.split('/').at(-1)
        const directory = file.path.slice(0, Math.max(0, file.path.length - name.length)).replace(/\/$/u, '')
        return `<button class="workspace-review-file${file.path === state.selected ? ' selected' : ''}" type="button" data-review-path="${escapeHtml(file.path)}">
          <span class="workspace-review-status ${status.tone}" title="${escapeHtml(translate(status.title))}">${status.label}</span>
          <span class="workspace-review-file-copy"><strong data-no-i18n>${escapeHtml(name)}</strong><small data-no-i18n>${escapeHtml(directory || '.')}</small></span>
          <span class="workspace-review-file-scopes">${file.staged ? '<i title="Staged">S</i>' : ''}${file.unstaged ? '<i title="Unstaged">W</i>' : ''}</span>
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
      host.innerHTML = `<div class="workspace-review-diff-empty"><span>±</span><strong>${escapeHtml(translate('Select a file to view its diff'))}</strong></div>`
      return
    }
    element('workspace-review-diff-path').textContent = file.path
    for (const button of element('workspace-review-diff-scopes')?.querySelectorAll('[data-diff-scope]') || []) {
      const scope = button.dataset.diffScope
      button.disabled = !file[scope]
      button.classList.toggle('active', state.diffScope === scope)
    }
    const action = element('workspace-review-action')
    action.textContent = state.diffScope === 'staged' ? translate('Unstage') : translate('Stage file')
    action.disabled = state.mutating || !file[state.diffScope]
    if (state.diffLoading) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span class="artifact-spinner"></span><strong>${escapeHtml(translate('Generating diff…'))}</strong></div>`
      return
    }
    if (state.diffError) {
      host.innerHTML = `<div class="workspace-review-diff-empty error"><span>!</span><strong>${escapeHtml(state.diffError)}</strong></div>`
      return
    }
    if (state.diff?.binary) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span>◫</span><strong>${escapeHtml(translate('Binary files cannot be compared line by line'))}</strong></div>`
      return
    }
    const rows = parseUnifiedDiff(state.diff?.content)
    host.innerHTML = `<div class="workspace-review-code" data-extension="${escapeHtml(file.path.split('.').at(-1) || '')}">${rows.map((row) => `<div class="diff-row ${row.kind}"><span class="diff-old">${row.oldLine ?? ''}</span><span class="diff-new">${row.newLine ?? ''}</span><span class="diff-sign">${row.kind === 'addition' ? '+' : row.kind === 'deletion' ? '−' : ''}</span><code>${escapeHtml(row.text)}</code></div>`).join('')}</div>${state.diff?.truncated ? `<div class="workspace-review-truncated">${escapeHtml(translate('The diff is large; only the first 4 MiB is shown.'))}</div>` : ''}`
  }

  return { bind, open, close, refresh, render }
}

function escapeHtml(value) {
  return String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#039;')
}
