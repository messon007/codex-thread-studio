import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  catalogListParams,
  mergeCatalogMetadata,
  reconcileStartedThreadCatalog,
  shouldRecoverCodexCatalog,
  turnStartParams,
} from './session-catalog.mjs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('Codex catalogs use the current state database while OpenCode parameters stay unchanged', () => {
  assert.deepEqual(catalogListParams('codex', { limit: 100 }), {
    limit: 100,
    useStateDbOnly: true,
  })
  assert.deepEqual(catalogListParams('opencode', { limit: 100 }), { limit: 100 })
})

test('Codex rollout recovery runs only for an empty catalog or a missing preferred session', () => {
  const catalog = [{ id: 'thread-1' }, { id: 'thread-2' }]

  assert.equal(shouldRecoverCodexCatalog([], null), true)
  assert.equal(shouldRecoverCodexCatalog(null, 'thread-1'), true)
  assert.equal(shouldRecoverCodexCatalog(catalog, 'thread-missing'), true)
  assert.equal(shouldRecoverCodexCatalog(catalog, 'thread-1'), false)
  assert.equal(shouldRecoverCodexCatalog(catalog, null), false)
})

test('catalog refreshes retain newly started sessions until the backend lists them', () => {
  const pending = [
    { id: 'new-one', cwd: '/work/one', turns: [{ id: 'not-catalog-metadata' }] },
    { id: 'new-two', cwd: '/work/two', turns: [] },
  ]
  const first = reconcileStartedThreadCatalog([{ id: 'existing' }], pending)

  assert.deepEqual(first.threads.map((thread) => thread.id), ['new-two', 'new-one', 'existing'])
  assert.deepEqual(first.retainedIds, ['new-two', 'new-one'])
  assert.deepEqual(first.confirmedIds, [])
  assert.equal(first.threads[0].turns, undefined)
  assert.equal(first.threads[1].turns, undefined)

  const confirmed = reconcileStartedThreadCatalog([
    { id: 'new-two', cwd: '/work/from-state-db' },
    { id: 'existing' },
  ], pending)
  assert.deepEqual(confirmed.threads.map((thread) => thread.id), ['new-one', 'new-two', 'existing'])
  assert.deepEqual(confirmed.retainedIds, ['new-one'])
  assert.deepEqual(confirmed.confirmedIds, ['new-two'])
  assert.equal(confirmed.threads[1].cwd, '/work/from-state-db')
})

test('started-session catalog reconciliation ignores invalid and duplicate provisional records', () => {
  const result = reconcileStartedThreadCatalog([], [
    null,
    { id: '' },
    { id: 'same', cwd: '/old' },
    { id: 'same', cwd: '/new' },
  ])
  assert.deepEqual(result.threads, [{ id: 'same', cwd: '/new', turns: undefined }])
  assert.deepEqual(result.retainedIds, ['same'])
})

test('Codex hydration preserves state-database catalog metadata', () => {
  const current = {
    id: 'thread-1',
    cwd: '/workspace/current',
    name: 'Current name',
    updatedAt: 200,
    recencyAt: 210,
    status: { type: 'notLoaded' },
  }
  const incoming = {
    id: 'thread-1',
    cwd: '/workspace/original',
    name: 'Original name',
    updatedAt: 100,
    recencyAt: 110,
    status: { type: 'idle' },
  }

  assert.deepEqual(mergeCatalogMetadata('codex', current, incoming), {
    ...current,
    status: { type: 'idle' },
  })
  assert.deepEqual(mergeCatalogMetadata('opencode', current, incoming), {
    ...current,
    ...incoming,
  })
})

test('only Codex turn starts receive the catalog working directory', () => {
  const params = { threadId: 'thread-1', cwd: '/caller/value' }
  const thread = { cwd: '/workspace/current' }
  assert.deepEqual(turnStartParams('codex', thread, params), {
    threadId: 'thread-1',
    cwd: '/workspace/current',
  })
  assert.deepEqual(turnStartParams('opencode', thread, params), params)
  assert.deepEqual(turnStartParams('codex', {}, params), params)
})

test('Studio applies DB catalogs and recovery only to Codex-compatible backends', () => {
  const codexCatalog = app.slice(
    app.indexOf('function fetchCodexCatalog('),
    app.indexOf('function requestCodexBackend('),
  )
  const openCodeCatalog = app.slice(
    app.indexOf('async function fetchOpenCodeCatalog('),
    app.indexOf('function fetchCodexCatalog('),
  )
  const recovery = app.slice(
    app.indexOf('function scheduleCodexCatalogRecovery('),
    app.indexOf('function installBackendCatalog('),
  )
  const focusRefresh = app.slice(
    app.indexOf('async function refreshActiveCodexCatalogOnFocus('),
    app.indexOf('function installBackendCatalog('),
  )

  assert.match(codexCatalog, /catalogListParams\('codex'/u)
  assert.doesNotMatch(openCodeCatalog, /useStateDbOnly|catalogListParams/u)
  assert.match(openCodeCatalog, /isSessionDirectoryHidden\(cwd, state\.hiddenSessionDirectories, state\.sessionDirectoryIgnore\)/u)
  assert.match(recovery, /isCodexBackend\(backend\)/u)
  assert.match(recovery, /shouldRecoverCodexCatalog\(catalog, preferredId\)/u)
  assert.match(recovery, /scheduleStudioIdleWork\(async \(\) =>/u)
  assert.doesNotMatch(recovery, /queueMicrotask/u)
  assert.match(recovery, /dispatchBackendRpc\(backend, 'thread\/read', \{ threadId: currentPreferredId, includeTurns: false \}\)/u)
  assert.ok(recovery.indexOf("dispatchBackendRpc(backend, 'thread/read'") < recovery.indexOf("dispatchBackendRpc(backend, 'thread/list'"))
  assert.match(recovery, /dispatchBackendRpc\(backend, 'thread\/list', \{ limit: 100 \}\)/u)
  assert.ok(recovery.indexOf("dispatchBackendRpc(backend, 'thread/list'") < recovery.indexOf('refreshBackendCatalog(backend)'))
  assert.match(recovery, /codexCatalogRecoveryStarted\.delete\(backend\)/u)
  assert.match(focusRefresh, /!isCodexBackend\(backend\)/u)
  assert.match(focusRefresh, /refreshBackendCatalog\(backend\)/u)
  assert.match(focusRefresh, /scheduleCodexCatalogRecovery\(backend\)/u)
  assert.match(app, /scheduleInactiveCatalogRefresh\(\)/u)
})

test('Studio binds canonical cwd to Codex turns without changing the OpenCode adapter', () => {
  const codexAdapter = app.slice(
    app.indexOf('function codexDispatchAdapter('),
    app.indexOf('const sessionDispatch'),
  )
  const openCodeAdapter = app.slice(
    app.indexOf(".register('opencode'"),
    app.indexOf('const commentSources'),
  )
  const composer = readFileSync(new URL('../ui-src/submission-controller.mts', import.meta.url), 'utf8')

  assert.match(codexAdapter, /turnStartParams\('codex', threadForRef\(ref\)/u)
  assert.doesNotMatch(openCodeAdapter, /turnStartParams/u)
  assert.match(composer, /turnStartParams\(\s*backendDescriptor\(backend\)\.kind/u)
  assert.match(composer, /dispatchBackendRpc\(backend, 'turn\/start'/u)
  assert.doesNotMatch(composer, /finally \{ button\.disabled = false \}/u)
})
