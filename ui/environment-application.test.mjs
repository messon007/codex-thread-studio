import test from 'node:test'
import assert from 'node:assert/strict'
import { createEnvironmentApplication } from './environment-application.mjs'

const base = { backend: 'codex', threadId: 'a', generation: 1, root: '/project', includeThread: false, profileRevision: 'r1', force: false, excludeTurns: true, initialTurnsPage: null }
function fixture(transport = async () => ({})) {
  const applied = new Set(), requests = new Map(), calls = [], disabled = []
  const apply = createEnvironmentApplication({ applied, requests, supports: backend => backend !== 'opencode',
    apply: async params => { calls.push(params); return transport(params) },
    compatibilityError: error => error.message === 'unsupported', disableTail: (...args) => disabled.push(args),
  })
  return { apply, applied, requests, calls, disabled }
}

test('environment application shares in-flight calls and skips already applied revisions', async () => {
  let resolve
  const f = fixture(() => new Promise(done => { resolve = done }))
  const first = f.apply(base), second = f.apply(base)
  assert.equal(f.calls.length, 1)
  resolve({ done: true })
  assert.deepEqual(await first, { done: true })
  assert.deepEqual(await second, { done: true })
  assert.equal(await f.apply(base), null)
  assert.equal(f.requests.size, 0)
})

test('backend, thread, generation, root and revision each distinguish cached application', async () => {
  const f = fixture()
  await f.apply(base)
  for (const change of [{ backend: 'ept-codex' }, { threadId: 'b' }, { generation: 2 }, { root: '/other' }, { profileRevision: 'r2' }, { force: true }]) await f.apply({ ...base, ...change })
  assert.equal(f.calls.length, 7)
  await f.apply({ ...base, backend: 'opencode' })
  await f.apply({ ...base, threadId: null })
  assert.equal(f.calls.length, 7)
})

test('unsupported incremental application retries once without initial page and records capability', async () => {
  const f = fixture(async params => { if (params.excludeTurns) throw new Error('unsupported'); return { done: true } })
  await f.apply({ ...base, includeThread: true, initialTurnsPage: { limit: 10 } })
  assert.deepEqual(f.calls.map(call => call.excludeTurns), [true, false])
  assert.deepEqual(f.calls[0].initialTurnsPage, { limit: 10 })
  assert.equal(Object.hasOwn(f.calls[1], 'initialTurnsPage'), false)
  assert.deepEqual(f.disabled, [['codex', 'a']])
  assert.equal(f.applied.size, 1)
})

test('ordinary failure does not retry or mark the revision applied', async () => {
  const f = fixture(async () => { throw new Error('permission denied') })
  await assert.rejects(f.apply(base), /permission denied/)
  assert.equal(f.calls.length, 1)
  assert.equal(f.applied.size, 0)
  assert.equal(f.requests.size, 0)
})
