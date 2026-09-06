import test from 'node:test'
import assert from 'node:assert/strict'
import { createStartedSessionCatalog } from './started-session-catalog.mjs'

function fixture() {
  const generations = new Map(), catalogs = {}, reports = [], refreshes = []
  const tracker = createStartedSessionCatalog({
    generations, key: (backend, id) => `${backend}:${id}`, catalog: backend => catalogs[backend] || [],
    merge: (backend, thread) => { catalogs[backend] = [...(catalogs[backend] || []).filter(item => item.id !== thread.id), thread] },
    report: (phase, details) => reports.push([phase, details]),
    refresh: async backend => { refreshes.push(backend) }, refreshError: (backend, error) => reports.push(['error', backend, error]),
    missingId: () => 'missing ID',
  })
  return { tracker, generations, catalogs, reports, refreshes }
}

test('new-session registration invalidates prior requests and keeps latest metadata without history', () => {
  const f = fixture()
  f.generations.set('codex', 3)
  const previousGeneration = f.generations.get('codex')
  f.tracker.remember('codex', { id: 'a', name: 'initial', turns: [{ id: 'large-history' }] }, 'new')
  assert.notEqual(f.generations.get('codex'), previousGeneration)
  f.catalogs.codex[0].name = 'renamed'
  assert.deepEqual(f.tracker.reconcile('codex', []), [{ id: 'a', name: 'renamed', turns: undefined }])
  f.tracker.reconcile('codex', [])
  assert.equal(f.reports.filter(([phase]) => phase === 'catalog-retained').length, 1)
  const actual = [{ id: 'a', name: 'from-server' }]
  assert.deepEqual(f.tracker.reconcile('codex', actual), actual)
  assert.deepEqual(f.tracker.reconcile('codex', []), [])
  assert.equal(f.reports.at(-1)[0], 'catalog-confirmed')
})

test('identical IDs stay isolated by backend and restart clears only its own entries', () => {
  const f = fixture()
  f.tracker.remember('codex', { id: 'same' }, 'new')
  f.tracker.remember('ept-codex', { id: 'same' }, 'fork')
  f.tracker.clearBackend('codex', 'restart')
  assert.deepEqual(f.tracker.reconcile('codex', []), [])
  assert.equal(f.tracker.reconcile('ept-codex', [])[0].id, 'same')
  assert.equal(f.tracker.forget('codex', 'same', 'deleted'), false)
  assert.equal(f.tracker.forget('ept-codex', 'same', 'deleted'), true)
})

test('catalog confirmation debounces at 800ms and cancellation prevents a refresh', t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const f = fixture()
  f.tracker.remember('opencode', { id: 'a' }, 'new')
  f.tracker.schedule('opencode', 'a')
  t.mock.timers.tick(500)
  f.tracker.schedule('opencode', 'a')
  t.mock.timers.tick(799)
  assert.deepEqual(f.refreshes, [])
  t.mock.timers.tick(1)
  assert.deepEqual(f.refreshes, ['opencode'])
  f.tracker.schedule('opencode', 'a')
  f.tracker.reconcile('opencode', [{ id: 'a' }])
  t.mock.timers.tick(800)
  assert.deepEqual(f.refreshes, ['opencode'])
  f.tracker.schedule('opencode', 'unknown')
  t.mock.timers.tick(800)
  assert.equal(f.refreshes.length, 1)
})

test('missing IDs are rejected without changing generations or merging catalog data', () => {
  const f = fixture()
  assert.throws(() => f.tracker.remember('codex', {}, 'new'), /missing ID/)
  assert.equal(f.generations.size, 0)
  assert.deepEqual(f.catalogs, {})
})

test('failed confirmation reports its backend and retains the provisional session without looping', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let requests = 0
  const errors = []
  const tracker = createStartedSessionCatalog({
    generations: new Map(), key: (backend, id) => `${backend}:${id}`, catalog: () => [], merge() {}, report() {},
    refresh: async () => { requests++; throw new Error('offline') },
    refreshError: (backend, error) => errors.push([backend, error.message]), missingId: () => 'missing ID',
  })
  tracker.remember('ept-codex', { id: 'a' }, 'fork')
  tracker.schedule('ept-codex', 'a')
  t.mock.timers.tick(800)
  await Promise.resolve()
  t.mock.timers.tick(10000)
  assert.equal(requests, 1)
  assert.deepEqual(errors, [['ept-codex', 'offline']])
  assert.equal(tracker.reconcile('ept-codex', [])[0].id, 'a')
})
