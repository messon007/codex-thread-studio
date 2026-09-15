import { parseUnifiedDiff, reviewFileStatus, visibleReviewCommits, visibleReviewFiles, createPendingGitReads } from './git-review-data.mjs'
export { parseUnifiedDiff, reviewFileStatus, visibleReviewCommits, visibleReviewFiles, createPendingGitReads } from './git-review-data.mjs'

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
        root: context.root, branch: '', mode: 'changes', files: [], selected: '', scope: 'all', diffScope: 'unstaged',
        commits: [], selectedCommit: '', commitFiles: [], selectedCommitFile: '', historyHasMore: false,
        filter: '', loading: false, historyLoading: false, commitLoading: false, diffLoading: false,
        error: '', historyError: '', commitError: '', diffError: '', diff: null,
      })
    }
    return states.get(context.key)
  }

  function bind() {
    element('workspace-review-refresh')?.addEventListener('click', () => refreshCurrentMode())
    element('workspace-review-filter')?.addEventListener('input', (event) => {
      const state = stateForCurrent()
      if (!state) return
      state.filter = event.target.value
      render(state)
    })
    element('workspace-review-modes')?.addEventListener('click', (event) => {
      const button = event.target.closest('[data-review-mode]')
      const state = stateForCurrent()
      if (!button || !state || button.dataset.reviewMode === state.mode) return
      state.mode = button.dataset.reviewMode
      state.filter = ''
      element('workspace-review-filter').value = ''
      render(state)
      if (state.mode === 'history' && !state.commits.length) loadHistory(state, { reset: true })
      else if (state.mode === 'changes' && !state.branch) refresh()
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
    element('workspace-review-commits')?.addEventListener('click', (event) => {
      const row = event.target.closest('[data-review-commit]')
      const state = stateForCurrent()
      if (!row || !state) return
      selectCommit(state, row.dataset.reviewCommit)
    })
    element('workspace-review-commit-files')?.addEventListener('click', (event) => {
      const row = event.target.closest('[data-review-commit-path]')
      const state = stateForCurrent()
      if (!row || !state) return
      state.selectedCommitFile = row.dataset.reviewCommitPath
      state.diff = null
      state.diffError = ''
      render(state)
      loadSelectedDiff(state)
    })
    element('workspace-review-history-more')?.addEventListener('click', () => {
      const state = stateForCurrent()
      if (state) loadHistory(state)
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
    if (state.mode === 'history') await loadHistory(state, { reset: true, quiet: Boolean(state.commits.length) })
    else await refresh({ quiet: Boolean(state.files.length) })
  }

  function close() { active = false }

  function refreshCurrentMode() {
    const state = stateForCurrent()
    if (!state) return
    if (state.mode === 'history') loadHistory(state, { reset: true })
    else refresh()
  }

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

  async function loadHistory(state = stateForCurrent(), { reset = false, quiet = false } = {}) {
    if (!state || state.historyLoading) return
    const requestKey = Symbol('history request')
    const offset = reset ? 0 : state.commits.length
    state.historyRequest = requestKey
    state.historyLoading = true
    state.historyError = ''
    if (!quiet) render(state)
    try {
      const result = await request('/studio/git/history', { root: state.root, offset, limit: 100 })
      if (state.historyRequest !== requestKey) return
      state.root = result.root || state.root
      state.commits = reset ? (result.commits || []) : [...state.commits, ...(result.commits || [])]
      state.historyHasMore = Boolean(result.hasMore)
      const visible = visibleReviewCommits(state.commits, state.filter)
      if (!visible.some((commit) => commit.hash === state.selectedCommit)) {
        state.selectedCommit = visible[0]?.hash || state.commits[0]?.hash || ''
      }
      render(state)
      if (state.selectedCommit && (reset || !state.commitFiles.length)) await loadCommitFiles(state)
    } catch (error) {
      if (state.historyRequest !== requestKey) return
      state.historyError = error.message
      if (reset) {
        state.commits = []
        state.selectedCommit = ''
        state.commitFiles = []
        state.diff = null
      }
    } finally {
      if (state.historyRequest === requestKey) {
        state.historyLoading = false
        render(state)
      }
    }
  }

  function selectCommit(state, commit) {
    if (!commit || commit === state.selectedCommit) return
    state.selectedCommit = commit
    state.selectedCommitFile = ''
    state.commitFiles = []
    state.diff = null
    state.diffError = ''
    render(state)
    loadCommitFiles(state)
  }

  async function loadCommitFiles(state = stateForCurrent()) {
    if (!state?.selectedCommit) return
    const requestKey = Symbol('commit request')
    const commit = state.selectedCommit
    state.commitRequest = requestKey
    state.commitLoading = true
    state.commitError = ''
    render(state)
    try {
      const result = await request('/studio/git/commit', { root: state.root, commit })
      if (state.commitRequest !== requestKey || state.selectedCommit !== commit) return
      state.commitFiles = result.files || []
      state.commitFilesTruncated = Boolean(result.truncated)
      if (!state.commitFiles.some((file) => file.path === state.selectedCommitFile)) {
        state.selectedCommitFile = state.commitFiles[0]?.path || ''
      }
      render(state)
      await loadSelectedDiff(state)
    } catch (error) {
      if (state.commitRequest !== requestKey) return
      state.commitFiles = []
      state.selectedCommitFile = ''
      state.commitError = error.message
      state.diff = null
    } finally {
      if (state.commitRequest === requestKey) {
        state.commitLoading = false
        render(state)
      }
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
    const history = state.mode === 'history'
    const body = history
      ? { root: state.root, commit: state.selectedCommit, path: file.path }
      : { root: state.root, path: file.path, scope: state.diffScope }
    const readKey = JSON.stringify([body, state.diffRevision || 0])
    state.diffRequest = requestKey
    state.diffLoading = true
    state.diffError = ''
    render(state)
    try {
      const result = await pendingDiffs(readKey, () => request(history ? '/studio/git/commit-diff' : '/studio/git/diff', body))
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
    if (!state || state.mode === 'history' || !file || state.mutating) return
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

  function selectedFile(state) {
    if (state?.mode === 'history') return state.commitFiles.find((file) => file.path === state.selectedCommitFile) || null
    return state?.files.find((file) => file.path === state.selected) || null
  }

  function render(state = stateForCurrent()) {
    if (!active || !state) return
    const history = state.mode === 'history'
    for (const button of element('workspace-review-modes')?.querySelectorAll('[data-review-mode]') || []) {
      const selected = button.dataset.reviewMode === state.mode
      button.classList.toggle('active', selected)
      button.setAttribute('aria-selected', String(selected))
    }
    element('workspace-review-filter').placeholder = translate(history ? 'Filter commits' : 'Filter changed files')
    element('workspace-review-scopes').classList.toggle('hidden', history)
    element('workspace-review-files').classList.toggle('hidden', history)
    element('workspace-review-history').classList.toggle('hidden', !history)
    element('workspace-review-count').textContent = translate(history ? '{count} commits' : '{count} changes', {
      count: history ? state.commits.length : state.files.length,
    })
    element('workspace-review-safety-title').textContent = translate(history ? 'Read-only history' : 'Safe review')
    element('workspace-review-safety-detail').textContent = translate(history
      ? 'Inspecting commits never changes the repository'
      : 'Stage or unstage files without discarding working-tree changes')
    if (history) renderHistory(state)
    else renderChanges(state)
    renderDiff(state)
  }

  function renderChanges(state) {
    const files = visibleReviewFiles(state.files, state.scope, state.filter)
    const counts = {
      all: state.files.length,
      unstaged: state.files.filter((file) => file.unstaged).length,
      staged: state.files.filter((file) => file.staged).length,
    }
    element('workspace-review-branch').textContent = state.branch || 'Git'
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
  }

  function renderHistory(state) {
    const commits = visibleReviewCommits(state.commits, state.filter)
    const list = element('workspace-review-commits')
    if (state.historyLoading && !state.commits.length) {
      list.innerHTML = `<div class="workspace-review-state">${escapeHtml(translate('Reading commit history…'))}</div>`
    } else if (state.historyError && !state.commits.length) {
      list.innerHTML = `<div class="workspace-review-state error"><strong>${escapeHtml(translate('Unable to read commit history'))}</strong><span>${escapeHtml(state.historyError)}</span></div>`
    } else if (!commits.length) {
      list.innerHTML = `<div class="workspace-review-state"><strong>${escapeHtml(translate(state.commits.length ? 'No commits match this filter' : 'This repository has no commits'))}</strong></div>`
    } else {
      list.innerHTML = commits.map((commit) => `<button class="workspace-review-commit${commit.hash === state.selectedCommit ? ' selected' : ''}" type="button" data-review-commit="${escapeHtml(commit.hash)}">
        <span class="workspace-review-commit-hash" data-no-i18n>${escapeHtml(commit.shortHash || commit.hash.slice(0, 7))}</span>
        <span class="workspace-review-commit-copy"><strong data-no-i18n>${escapeHtml(commit.subject || translate('Untitled commit'))}</strong><small><span data-no-i18n>${escapeHtml(commit.authorName || commit.authorEmail || translate('Unknown author'))}</span> · ${escapeHtml(formatCommitDate(commit.authoredAt))}</small></span>
      </button>`).join('')
    }
    const more = element('workspace-review-history-more')
    more.classList.toggle('hidden', !state.historyHasMore)
    more.disabled = state.historyLoading
    more.textContent = translate(state.historyLoading ? 'Loading older commits…' : 'Load older commits')

    const commit = state.commits.find((entry) => entry.hash === state.selectedCommit)
    element('workspace-review-commit-title').textContent = commit?.subject || translate('Commit files')
    element('workspace-review-commit-count').textContent = translate('{count} files', { count: state.commitFiles.length })
    const files = element('workspace-review-commit-files')
    if (state.commitLoading) {
      files.innerHTML = `<div class="workspace-review-state">${escapeHtml(translate('Reading commit files…'))}</div>`
    } else if (state.commitError) {
      files.innerHTML = `<div class="workspace-review-state error"><strong>${escapeHtml(translate('Unable to read commit'))}</strong><span>${escapeHtml(state.commitError)}</span></div>`
    } else if (!state.selectedCommit) {
      files.innerHTML = `<div class="workspace-review-state"><strong>${escapeHtml(translate('Select a commit to view changed files'))}</strong></div>`
    } else if (!state.commitFiles.length) {
      files.innerHTML = `<div class="workspace-review-state"><strong>${escapeHtml(translate('This commit has no file changes'))}</strong></div>`
    } else {
      files.innerHTML = state.commitFiles.map((file) => {
        const status = reviewFileStatus(file)
        const name = file.path.split('/').at(-1)
        const directory = file.path.slice(0, Math.max(0, file.path.length - name.length)).replace(/\/$/u, '')
        return `<button class="workspace-review-file${file.path === state.selectedCommitFile ? ' selected' : ''}" type="button" data-review-commit-path="${escapeHtml(file.path)}">
          <span class="workspace-review-status ${status.tone}" title="${escapeHtml(translate(status.title))}">${status.label}</span>
          <span class="workspace-review-file-copy"><strong data-no-i18n>${escapeHtml(name)}</strong><small data-no-i18n>${escapeHtml(file.previousPath ? `${file.previousPath} → ${file.path}` : directory || '.')}</small></span>
        </button>`
      }).join('') + (state.commitFilesTruncated ? `<div class="workspace-review-truncated">${escapeHtml(translate('The file list is large; only the first 1000 files are shown.'))}</div>` : '')
    }
  }

  function renderDiff(state) {
    const file = selectedFile(state)
    const history = state.mode === 'history'
    const header = element('workspace-review-diff-header')
    header.classList.toggle('hidden', !file)
    const host = element('workspace-review-diff')
    if (!file) {
      host.innerHTML = `<div class="workspace-review-diff-empty"><span>±</span><strong>${escapeHtml(translate(history ? 'Select a commit file to view its diff' : 'Select a file to view its diff'))}</strong></div>`
      return
    }
    element('workspace-review-diff-path').textContent = file.path
    element('workspace-review-diff-scopes').classList.toggle('hidden', history)
    for (const button of element('workspace-review-diff-scopes')?.querySelectorAll('[data-diff-scope]') || []) {
      const scope = button.dataset.diffScope
      button.disabled = !file[scope]
      button.classList.toggle('active', state.diffScope === scope)
    }
    const action = element('workspace-review-action')
    const openFile = element('workspace-review-open-file')
    action.classList.toggle('hidden', history)
    openFile.classList.toggle('hidden', history)
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

function formatCommitDate(value) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? String(value || '') : date.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })
}
