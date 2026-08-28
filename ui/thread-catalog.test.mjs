import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { installBackendRegistry } from './backends.mjs'

import {
  catalogCountsWithAttention,
  catalogTimestamp,
  compactSidebarText,
  filterCatalogEntries,
  groupCatalogEntries,
  isCatalogCacheFresh,
  isSessionDirectoryHidden,
  normalizeHiddenSessionDirectories,
  normalizeSessionDirectoryIgnore,
  syncCatalogSelection,
  threadCatalogKey,
} from './thread-catalog.mjs'

const catalogs = {
  codex: [
    { id: 'same-id', name: 'Codex task', cwd: '/work/codex', status: 'idle', updatedAt: 20 },
    { id: 'running-cx', name: 'Running Codex', cwd: '/work/shared', status: { type: 'running' }, updatedAt: '2026-07-24T08:00:00Z' },
  ],
  'company-codex': [
    { id: 'same-id', name: 'Company Codex task', cwd: '/work/company', status: 'idle', updatedAt: 25 },
  ],
  opencode: [
    { id: 'same-id', name: 'OpenCode task', cwd: '/work/opencode', status: 'idle', updatedAt: 30 },
  ],
}

test.beforeEach(() => installBackendRegistry([
  { id: 'codex', name: 'Codex', tag: 'CX', kind: 'codex' },
  { id: 'company-codex', name: 'Company Codex', tag: 'WK', kind: 'codex' },
  { id: 'opencode', name: 'OpenCode', tag: 'OC', kind: 'opencode' },
]))
test.afterEach(() => installBackendRegistry())

test('keeps equal IDs from different backends as separate sessions', () => {
  const entries = filterCatalogEntries(catalogs)
  assert.equal(entries.length, 4)
  assert.deepEqual(
    entries.filter(({ thread }) => thread.id === 'same-id').map(({ backend }) => backend).sort(),
    ['codex', 'company-codex', 'opencode'],
  )
})

test('normalizes second and millisecond epoch timestamps for cross-backend ordering', () => {
  assert.equal(catalogTimestamp(1_784_881_800), 1_784_881_800_000)
  assert.equal(catalogTimestamp(1_784_881_800_123), 1_784_881_800_123)
})

test('treats the Codex validation second as uncertain when checking cached history', () => {
  const validatedAt = 1_784_881_800_700
  assert.equal(isCatalogCacheFresh(1_784_881_800, validatedAt, { coarse: true }), false)
  assert.equal(isCatalogCacheFresh(1_784_881_800, validatedAt), true)
  assert.equal(isCatalogCacheFresh(1_784_881_799, validatedAt, { coarse: true }), true)
  assert.equal(isCatalogCacheFresh(1_784_881_800_600, validatedAt), true)
  assert.equal(isCatalogCacheFresh(1_784_881_800_800, validatedAt), false)
  assert.equal(isCatalogCacheFresh(null, validatedAt, { coarse: true }), true)
})

test('reports deck-style all, active, and attention counts', () => {
  const attention = new Set([threadCatalogKey('opencode', 'same-id')])
  assert.deepEqual(catalogCountsWithAttention(catalogs, attention), { all: 4, active: 1, attention: 1 })
  assert.deepEqual(filterCatalogEntries(catalogs, { filter: 'active' }).map(({ thread }) => thread.id), ['running-cx'])
  assert.deepEqual(filterCatalogEntries(catalogs, { filter: 'attention', attention }).map(({ thread }) => thread.name), ['OpenCode task'])
})

test('attention mode orders loaded sessions by catalog update time', () => {
  const attention = new Set([
    threadCatalogKey('codex', 'same-id'),
    threadCatalogKey('opencode', 'same-id'),
    threadCatalogKey('codex', 'running-cx'),
  ])
  assert.deepEqual(
    filterCatalogEntries(catalogs, { filter: 'attention', attention }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:running-cx', 'opencode:same-id', 'codex:same-id'],
  )
})

test('all and active modes keep catalog order', () => {
  assert.deepEqual(
    filterCatalogEntries(catalogs, { filter: 'all' }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:same-id', 'codex:running-cx', 'company-codex:same-id', 'opencode:same-id'],
  )
  assert.deepEqual(
    filterCatalogEntries(catalogs, { filter: 'active' }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:running-cx'],
  )
})

test('search includes the backend tag and project directory', () => {
  assert.deepEqual(filterCatalogEntries(catalogs, { search: 'OC' }).map(({ thread }) => thread.name), ['OpenCode task'])
  assert.deepEqual(filterCatalogEntries(catalogs, { search: 'WK' }).map(({ thread }) => thread.name), ['Company Codex task'])
  assert.deepEqual(filterCatalogEntries(catalogs, { search: '/work/codex' }).map(({ thread }) => thread.name), ['Codex task'])
})

test('compacts sidebar copy without splitting Unicode characters', () => {
  assert.equal(compactSidebarText('  A\n  short\t title  '), 'A short title')
  assert.equal(compactSidebarText('A😀BC', 4), 'A😀BC')
  assert.equal(compactSidebarText('A😀BCD', 4), 'A😀B…')
  assert.equal(compactSidebarText(`short${'x'.repeat(100_000)}`, 10), 'shortxxxx…')
})

test('updates only the selected catalog row for a session switch', () => {
  const rows = [
    fakeCatalogRow('codex', 'one', true),
    fakeCatalogRow('codex', 'two', false),
    fakeCatalogRow('opencode', 'two', false),
  ]
  syncCatalogSelection(rows, 'codex', 'two')
  assert.deepEqual(rows.map((row) => row.active), [false, true, false])
  syncCatalogSelection(rows, 'opencode', 'two')
  assert.deepEqual(rows.map((row) => row.active), [false, false, true])
})

test('hides configured directories and all descendants without deleting catalog data', () => {
  const hiddenDirectories = ['/work/codex/', 'C:\\Users\\Rui\\Archive']
  assert.deepEqual(normalizeHiddenSessionDirectories(hiddenDirectories), ['/work/codex', 'C:/Users/Rui/Archive'])
  assert.equal(isSessionDirectoryHidden('/work/codex/subproject', hiddenDirectories), true)
  assert.equal(isSessionDirectoryHidden('/work/codex-other', hiddenDirectories), false)
  assert.equal(isSessionDirectoryHidden('c:/users/rui/archive/project', hiddenDirectories), true)
  assert.deepEqual(
    filterCatalogEntries(catalogs, { hiddenDirectories }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:running-cx', 'company-codex:same-id', 'opencode:same-id'],
  )
  assert.deepEqual(
    catalogCountsWithAttention(catalogs, new Set(), hiddenDirectories),
    { all: 3, active: 1, attention: 0 },
  )
})

test('applies ordered gitignore-style directory rules with negation and globstars', () => {
  const patterns = [
    '# generated projects',
    '/home/rui/desktop/lisource/aswcodex/',
    '**/node_modules/',
    'scratch-*',
    '!/home/rui/desktop/lisource/aswcodex/keep-this/',
  ]
  assert.equal(normalizeSessionDirectoryIgnore(patterns).length, 4)
  assert.equal(isSessionDirectoryHidden('/home/rui/desktop/lisource/aswcodex', [], patterns), true)
  assert.equal(isSessionDirectoryHidden('/home/rui/desktop/lisource/aswcodex/project-a', [], patterns), true)
  assert.equal(isSessionDirectoryHidden('/home/rui/desktop/lisource/aswcodex/keep-this/subproject', [], patterns), false)
  assert.equal(isSessionDirectoryHidden('/work/app/node_modules/library', [], patterns), true)
  assert.equal(isSessionDirectoryHidden('/work/scratch-demo/nested', [], patterns), true)
  assert.equal(isSessionDirectoryHidden('/work/production', [], patterns), false)
})

test('groups regular views by full directory and uses the last path level as the label', () => {
  const groups = groupCatalogEntries(filterCatalogEntries(catalogs))
  assert.deepEqual(groups.map(({ cwd, name }) => [cwd, name]), [
    ['/work/codex', 'codex'],
    ['/work/shared', 'shared'],
    ['/work/company', 'company'],
    ['/work/opencode', 'opencode'],
  ])
})

test('the sidebar contract has one create button, directory groups, and no backend tabs', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.equal((html.match(/id="new-thread"/g) || []).length, 1)
  assert.match(html, /id="new-thread-backend"/)
  assert.doesNotMatch(html, /backend-switcher/)
  assert.doesNotMatch(source, /projectGroups\(/)
  assert.match(source, /groupCatalogEntries\(entries\)/)
  assert.match(source, /<small data-no-i18n title="\\?\$\{escapeHtml\(thread\.cwd/)
  assert.match(html, /data-filter="attention"[^>]*>Attention <span id="count-attention">0<\/span>/)
  assert.doesNotMatch(html, /id="thread-meta"/)
  assert.match(source, /function openThreadInfo\(\)[\s\S]*CLI Version/)
  assert.match(styles, /\.thread-group-heading strong[^}]*text-transform: uppercase/)
  assert.match(styles, /\.backend-tag \{[^}]*background: var\(--panel-strong\)/)
  assert.match(styles, /\.thread-row\.active \.backend-tag/)
})

test('session selection renders a valid cache before performing a first history load', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('async function selectThread(')
  const end = source.indexOf('\nfunction markThreadLoaded', start)
  const selectThread = source.slice(start, end)
  assert.ok(selectThread.indexOf('freshThreadModel(') < selectThread.indexOf('renderTranscript()'))
  assert.ok(selectThread.indexOf('setNativeError(null)') < selectThread.indexOf('if (cached)'))
  assert.ok(selectThread.indexOf('renderTranscript()') < selectThread.indexOf('schedulePreferencesPersist()'))
  assert.ok(selectThread.indexOf('if (cached)') < selectThread.indexOf('await resumeThread(id,'))
  assert.match(selectThread, /syncThreadListSelection\(\)/u)
  assert.doesNotMatch(selectThread, /renderThreadList\(\)/u)
  assert.doesNotMatch(selectThread, /thread\/unsubscribe/)
})

test('catalog refreshes do not rerender a still-selected fresh transcript', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('async function loadThreads(')
  const end = source.indexOf('\nfunction setActiveThreads', start)
  const loadThreads = source.slice(start, end)
  assert.match(loadThreads, /preferredLoad = cached[\s\S]*loadSelectedSessionCompanions/u)
  assert.match(loadThreads, /preferredMissingFromCatalog[\s\S]*thread\/read'[\s\S]*threadId: preferred, includeTurns: false/u)
  assert.match(loadThreads, /preferredMissingFromCatalog && isCodexBackend\(backend\)[\s\S]*mergeThreadIntoCatalog\(backend, knownPreferred\)/u)
  assert.match(loadThreads, /backend === 'opencode' && preferredMissingFromCatalog[\s\S]*invalidateThreadModel\(backend, preferred\)/u)
  assert.match(loadThreads, /await preferredLoad[\s\S]*preferredMissingFromCatalog && isCodexBackend\(backend\)[\s\S]*threadId: preferred, includeTurns: false/u)
  assert.match(loadThreads, /const preserveMissingPreferred = preferredLoad[\s\S]*!preferredMissingFromCatalog \|\| isCodexBackend\(backend\)[\s\S]*const nextId = preserveMissingPreferred/u)
  assert.match(loadThreads, /await preferredLoad[\s\S]*if \(preferredUsedCache && !freshThreadModel\(backend, nextId\)\)[\s\S]*selectThread/u)
  assert.match(loadThreads, /state\.selectedId !== preferred[\s\S]*return true/u)
  assert.match(loadThreads, /else if \(nextId && \(state\.selectedId !== nextId \|\| !freshThreadModel\(backend, nextId\)\)\)[\s\S]*selectThread/u)
})

test('session switches prepare their own transcript position before rendering cached content', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const selectStart = source.indexOf('async function selectThread(')
  const selectEnd = source.indexOf('\nfunction markThreadLoaded', selectStart)
  const selectThread = source.slice(selectStart, selectEnd)
  const backendStart = source.indexOf('async function switchBackend(')
  const backendEnd = source.indexOf('\nfunction applyBackendCopy', backendStart)
  const switchBackend = source.slice(backendStart, backendEnd)

  assert.ok(selectThread.indexOf('prepareTranscriptViewForSelection(') < selectThread.indexOf('renderTranscript()'))
  assert.ok(switchBackend.indexOf('prepareTranscriptViewForSelection(') < switchBackend.indexOf('renderTranscript()'))
  assert.match(selectThread, /historyReady: Boolean\(cached\),\s*historyComplete: cached\?\.model\?\.historyComplete !== false/u)
  assert.match(switchBackend, /historyReady: Boolean\(cached\),\s*historyComplete: cached\?\.model\?\.historyComplete !== false/u)
})

function fakeCatalogRow(backend, threadId, active) {
  const row = {
    dataset: { backend, threadId },
    active,
    classList: {
      toggle(name, enabled) {
        assert.equal(name, 'active')
        row.active = enabled
      },
    },
  }
  return row
}
