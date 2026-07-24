import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  catalogCountsWithAttention,
  filterCatalogEntries,
  groupCatalogEntries,
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

test('reports deck-style all, active, and attention counts', () => {
  const attention = new Set([threadCatalogKey('opencode', 'same-id')])
  assert.deepEqual(catalogCountsWithAttention(catalogs, attention), { all: 3, active: 1, attention: 1 })
  assert.deepEqual(filterCatalogEntries(catalogs, { filter: 'active' }).map(({ thread }) => thread.id), ['running-cx'])
  assert.deepEqual(filterCatalogEntries(catalogs, { filter: 'attention', attention }).map(({ thread }) => thread.name), ['OpenCode task'])
})

test('attention mode prioritizes the latest local interaction', () => {
  const activity = {
    [threadCatalogKey('codex', 'same-id')]: 100,
    [threadCatalogKey('opencode', 'same-id')]: 300,
    [threadCatalogKey('codex', 'running-cx')]: 200,
  }
  const attention = new Set(Object.keys(activity))
  assert.deepEqual(
    filterCatalogEntries(catalogs, { filter: 'attention', activity, attention }).map(({ backend, thread }) => `${backend}:${thread.id}`),
    ['opencode:same-id', 'codex:running-cx', 'codex:same-id'],
  )
})

test('search includes the backend tag and project directory', () => {
  assert.deepEqual(filterCatalogEntries(catalogs, { search: 'OC' }).map(({ thread }) => thread.name), ['OpenCode task'])
  assert.deepEqual(filterCatalogEntries(catalogs, { search: '/work/codex' }).map(({ thread }) => thread.name), ['Codex task'])
})

test('groups regular views by full directory and uses the last path level as the label', () => {
  const groups = groupCatalogEntries(filterCatalogEntries(catalogs))
  assert.deepEqual(groups.map(({ cwd, name }) => [cwd, name]), [
    ['/work/shared', 'shared'],
    ['/work/opencode', 'opencode'],
    ['/work/codex', 'codex'],
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
  const end = source.indexOf('\nfunction touchThreadActivity', start)
  const selectThread = source.slice(start, end)
  assert.ok(selectThread.indexOf('freshThreadModel(') < selectThread.indexOf('renderTranscript()'))
  assert.ok(selectThread.indexOf('if (cached)') < selectThread.indexOf('await resumeThread(id)'))
  assert.doesNotMatch(selectThread, /thread\/unsubscribe/)
})
