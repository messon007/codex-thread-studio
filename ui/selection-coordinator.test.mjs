import test from 'node:test'
import assert from 'node:assert/strict'
import { awaitBackendSelection, completeSessionSelection } from './selection-coordinator.mjs'

function deferred() { let resolve; const promise = new Promise(done => { resolve = done }); return { promise, resolve } }
function fixture(overrides = {}) {
  const calls = []
  return { calls, options: { fresh: false, cached: true, codex: true,
    mapLoad: Promise.resolve(), environmentLoad: Promise.resolve({ configured: true, root: '/project', revision: 'r1' }),
    isCurrent: () => true, status: value => calls.push(value),
    resume: async options => calls.push(options), bootstrap: () => calls.push('bootstrap'), ...overrides,
  } }
}

test('fresh selection waits for companions but never resumes history', async () => {
  const map = deferred(), env = deferred()
  const f = fixture({ fresh: true, mapLoad: map.promise, environmentLoad: env.promise })
  const done = completeSessionSelection(f.options)
  assert.deepEqual(f.calls, ['cached'])
  env.resolve(null)
  await Promise.resolve()
  assert.deepEqual(f.calls, ['cached'])
  map.resolve()
  await done
  assert.deepEqual(f.calls, ['cached', 'bootstrap'])
})

test('Codex loads environment before one resume and forwards its revision', async () => {
  const env = deferred()
  const f = fixture({ environmentLoad: env.promise })
  const done = completeSessionSelection(f.options)
  assert.deepEqual(f.calls, ['checking'])
  env.resolve({ configured: true, root: '/project', revision: 'r2' })
  await done
  assert.deepEqual(f.calls, ['checking', { environmentRoot: '/project', environmentRevision: 'r2' }, 'bootstrap'])
})

test('OpenCode starts history without waiting for environment, then waits before bootstrap', async () => {
  const env = deferred()
  const f = fixture({ codex: false, cached: false, environmentLoad: env.promise })
  const done = completeSessionSelection(f.options)
  assert.deepEqual(f.calls, [{ environmentRoot: '', environmentRevision: '' }])
  await Promise.resolve()
  assert.equal(f.calls.length, 1)
  env.resolve(null)
  await done
  assert.equal(f.calls.at(-1), 'bootstrap')
})

test('a changed selection suppresses resume or bootstrap at each async boundary', async () => {
  for (const phase of ['fresh', 'before-resume', 'after-resume']) {
    const f = fixture({ fresh: phase === 'fresh' })
    let current = phase === 'after-resume'
    f.options.isCurrent = () => current
    f.options.resume = async () => { f.calls.push('resume'); current = false }
    await completeSessionSelection(f.options)
    assert.equal(f.calls.includes('bootstrap'), false)
    assert.equal(f.calls.includes('resume'), phase === 'after-resume')
  }
})

test('cross-backend selection waits for the ready handlers initial load and fresh cache', async () => {
  const calls = [], load = deferred()
  const done = awaitBackendSelection({
    switchBackend: async () => calls.push('switch'),
    waitFor: async (predicate, timeout) => { assert.equal(predicate(), true); calls.push(timeout) },
    ready: () => true, initialLoad: () => load.promise,
    catalogContainsSession: () => true, freshSelection: () => true,
  })
  await Promise.resolve(); await Promise.resolve()
  assert.deepEqual(calls, ['switch', 15000])
  load.resolve()
  await done
  assert.deepEqual(calls, ['switch', 15000, 15000, 30000])
})

test('history failure propagates without bootstrapping or starting another request', async () => {
  const f = fixture()
  let requests = 0
  f.options.resume = async () => { requests++; throw new Error('offline') }
  await assert.rejects(completeSessionSelection(f.options), /offline/)
  assert.equal(requests, 1)
  assert.equal(f.calls.includes('bootstrap'), false)
})
