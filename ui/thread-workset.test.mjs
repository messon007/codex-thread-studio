import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  addLoadedThread,
  preserveCatalogActivity,
  restoreCatalogThreadActivity,
  updateCatalogThreadActivity,
  updateLoadedCatalogTimestamp,
} from './thread-workset.mjs'

test('loaded workset keeps equal IDs from different backends and is idempotent', () => {
  const loaded = new Set()
  assert.equal(addLoadedThread(loaded, 'codex', 'same-id'), true)
  assert.equal(addLoadedThread(loaded, 'opencode', 'same-id'), true)
  assert.equal(addLoadedThread(loaded, 'codex', 'same-id'), false)
  assert.deepEqual([...loaded], ['codex:same-id', 'opencode:same-id'])
})

test('only loaded sessions receive a dynamic catalog timestamp', () => {
  const catalogs = {
    codex: [{ id: 'loaded', updatedAt: 10 }, { id: 'cold', updatedAt: 20 }],
    opencode: [],
  }
  const loaded = new Set(['codex:loaded'])
  assert.equal(updateLoadedCatalogTimestamp(catalogs, loaded, 'codex', 'cold', 30), false)
  assert.equal(updateLoadedCatalogTimestamp(catalogs, loaded, 'codex', 'loaded', 40), true)
  assert.deepEqual(catalogs.codex, [
    { id: 'loaded', updatedAt: 10, activityAt: 40 },
    { id: 'cold', updatedAt: 20 },
  ])
})

test('catalog refreshes preserve a newer runtime activity clock without replacing backend metadata', () => {
  const refreshed = preserveCatalogActivity(
    [{ id: 'thread-1', status: 'idle', updatedAt: 50 }],
    [{ id: 'thread-1', status: 'active', updatedAt: 10, activityAt: 60 }],
  )
  assert.deepEqual(refreshed, [{ id: 'thread-1', status: 'idle', updatedAt: 50, activityAt: 60 }])
})

test('catalog activity updates status and time as one reversible mutation', () => {
  const catalogs = {
    codex: [{ id: 'thread-1', status: 'idle', updatedAt: 10 }],
  }
  const change = updateCatalogThreadActivity(catalogs, 'codex', 'thread-1', {
    status: 'active',
    timestamp: 40,
  })
  assert.equal(change.statusChanged, true)
  assert.equal(change.timestampChanged, true)
  assert.deepEqual(catalogs.codex[0], { id: 'thread-1', status: 'active', updatedAt: 10, activityAt: 40 })
  assert.ok(restoreCatalogThreadActivity(catalogs, change))
  assert.deepEqual(catalogs.codex[0], { id: 'thread-1', status: 'idle', updatedAt: 10 })
})

test('an authoritative lifecycle update prevents an optimistic activity rollback', () => {
  const catalogs = {
    codex: [{ id: 'thread-1', status: 'idle', updatedAt: 10 }],
  }
  const optimistic = updateCatalogThreadActivity(catalogs, 'codex', 'thread-1', {
    status: 'active',
    timestamp: 40,
  })
  updateCatalogThreadActivity(catalogs, 'codex', 'thread-1', {
    status: { type: 'active' },
    timestamp: 41,
  })
  assert.equal(restoreCatalogThreadActivity(catalogs, optimistic), null)
  assert.deepEqual(catalogs.codex[0], {
    id: 'thread-1',
    status: { type: 'active' },
    updatedAt: 10,
    activityAt: 41,
  })
})

test('Studio starts with an empty runtime workset and adds a session after loading history', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /state\.attentionThreads = new Set\(\)/)
  assert.doesNotMatch(source, /attentionThreads: \[\.\.\.state\.attentionThreads\]/)
  const resume = source.slice(
    source.indexOf('async function resumeThreadUncached('),
    source.indexOf('async function refreshSelectedThread('),
  )
  assert.ok(resume.indexOf('cacheThreadModel(') > resume.indexOf("rpc('thread/resume'"))
  assert.match(source, /function cacheThreadModel[\s\S]*markThreadLoaded\(backend, id\)/)
  assert.match(source, /function installBackendCatalog[\s\S]*preserveCatalogActivity\([\s\S]*function refreshRouterCatalogs/)
  assert.match(source, /function setActiveThreads[\s\S]*preserveCatalogActivity\([\s\S]*function sidebarThreadsForBackend/)
})

test('turn boundaries update catalog status and activity without streaming-list churn', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const codexLifecycle = readFileSync(new URL('../ui-src/background-sessions.mts', import.meta.url), 'utf8')
  const catalogLifecycle = source.slice(
    source.indexOf('function updateCodexCatalogActivity('),
    source.indexOf('\nfunction updateLoadedThreadTimestamp(', source.indexOf('function updateCodexCatalogActivity(')),
  )
  assert.match(codexLifecycle, /updateCodexCatalogActivity\(backend, message, event, threadId\)/)
  assert.match(catalogLifecycle, /event\.method === 'turn\/started'[\s\S]*status: 'active', touch: true/)
  assert.match(catalogLifecycle, /event\.method !== 'turn\/completed'[\s\S]*status, touch: true/)
  assert.match(catalogLifecycle, /payload\.type === 'session\.idle'[\s\S]*status: 'idle', touch: true/)
  assert.doesNotMatch(catalogLifecycle, /message\.part\.delta[\s\S]*touch: true/)
})

test('composer and queued sends optimistically activate one catalog row and roll back rejected starts', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const submission = readFileSync(new URL('../ui-src/submission-controller.mts', import.meta.url), 'utf8')
  const queue = submission.slice(
    submission.indexOf('async function runNextQueuedMessage('),
    submission.indexOf('\nfunction pauseMessageQueue('),
  )
  const composer = submission.slice(
    submission.indexOf('async function sendComposer('),
    submission.indexOf('\nasync function runNextQueuedMessage('),
  )
  assert.match(queue, /setCatalogThreadActivity\(ref\.backend, ref\.id, \{ status: 'active', touch: true \}\)/)
  assert.match(queue, /if \(!accepted\) rollbackCatalogThreadActivity\(catalogActivity\)/)
  assert.match(composer, /setCatalogThreadActivity\(backend, threadId, \{ status: 'active', touch: true \}\)/)
  assert.match(composer, /if \(!turnAccepted\) rollbackCatalogThreadActivity\(catalogActivity\)/)
  assert.match(source, /function refreshCatalogActivity[\s\S]*state\.filter === 'attention'[\s\S]*patchThreadCatalogRow\(change\.backend, change\.thread\)/)
})
