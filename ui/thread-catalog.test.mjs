import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  catalogCountsWithAttention,
  catalogTimestamp,
  filterCatalogEntries,
  groupCatalogEntries,
  isSessionDirectoryHidden,
  normalizeHiddenSessionDirectories,
  normalizeSessionDirectoryIgnore,
  threadCatalogKey,
} from './thread-catalog.mjs'

const catalogs = {
  codex: [
    { id: 'same-id', name: 'Codex task', cwd: '/work/codex', status: 'idle', updatedAt: 20 },
    { id: 'running-cx', name: 'Running Codex', cwd: '/work/shared', status: { type: 'running' }, updatedAt: '2026-07-24T08:00:00Z' },
  ],
  opencode: [
    { id: 'same-id', name: 'OpenCode task', cwd: '/work/opencode', status: 'idle', updatedAt: 30 },
  ],
}

test('keeps equal IDs from different backends as separate sessions', () => {
  const entries = filterCatalogEntries(catalogs)
  assert.equal(entries.length, 3)
  assert.deepEqual(
    entries.filter(({ thread }) => thread.id === 'same-id').map(({ backend }) => backend).sort(),
    ['codex', 'opencode'],
  )
})

test('normalizes second and millisecond epoch timestamps for cross-backend ordering', () => {
  assert.equal(catalogTimestamp(1_784_881_800), 1_784_881_800_000)
  assert.equal(catalogTimestamp(1_784_881_800_123), 1_784_881_800_123)
})

test('reports deck-style all, active, and attention counts', () => {
  const attention = new Set([threadCatalogKey('opencode', 'same-id')])
  assert.deepEqual(catalogCountsWithAttention(catalogs, attention), { all: 3, active: 1, attention: 1 })
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
    ['codex:same-id', 'codex:running-cx', 'opencode:same-id'],
  )
  assert.deepEqual(
    filterCatalogEntries(catalogs, { filter: 'active' }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:running-cx'],
  )
})

test('search includes the backend tag and project directory', () => {
  assert.deepEqual(filterCatalogEntries(catalogs, { search: 'OC' }).map(({ thread }) => thread.name), ['OpenCode task'])
  assert.deepEqual(filterCatalogEntries(catalogs, { search: '/work/codex' }).map(({ thread }) => thread.name), ['Codex task'])
})

test('hides configured directories and all descendants without deleting catalog data', () => {
  const hiddenDirectories = ['/work/codex/', 'C:\\Users\\Rui\\Archive']
  assert.deepEqual(normalizeHiddenSessionDirectories(hiddenDirectories), ['/work/codex', 'C:/Users/Rui/Archive'])
  assert.equal(isSessionDirectoryHidden('/work/codex/subproject', hiddenDirectories), true)
  assert.equal(isSessionDirectoryHidden('/work/codex-other', hiddenDirectories), false)
  assert.equal(isSessionDirectoryHidden('c:/users/rui/archive/project', hiddenDirectories), true)
  assert.deepEqual(
    filterCatalogEntries(catalogs, { hiddenDirectories }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['codex:running-cx', 'opencode:same-id'],
  )
  assert.deepEqual(
    catalogCountsWithAttention(catalogs, new Set(), hiddenDirectories),
    { all: 2, active: 1, attention: 0 },
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
  assert.match(html, /data-filter="attention"[^>]*>待处理 <span id="count-attention">0<\/span>/)
  assert.doesNotMatch(html, /id="thread-meta"/)
  assert.match(source, /function openThreadInfo\(\)[\s\S]*CLI 版本/)
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
  assert.ok(selectThread.indexOf('if (cached)') < selectThread.indexOf('await resumeThread(id)'))
  assert.doesNotMatch(selectThread, /thread\/unsubscribe/)
})
