import assert from 'node:assert/strict'
import test from 'node:test'
import { SessionMapCoordinator } from './session-map-coordination.mjs'
import { normalizeSessionMap } from './session-map.mjs'
const map = (revision = 1, id = 'map') => normalizeSessionMap({ id, backend: 'codex', threadId: 'a', revision })
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject } }
function fixture() {
  const state = { sessionMaps: new Map(), sessionMapLoads: new Map(), sessionMapSync: new Map() }, changed = []
  return { state, changed, flow: new SessionMapCoordinator(state, key => changed.push(key)) }
}
test('Map loads share one flight; cached missing maps and 404s do not trigger repeated requests', async () => {
  const { flow, state } = fixture(), gate = deferred()
  const first = flow.load('codex:a', () => gate.promise)
  assert.equal(flow.load('codex:a', () => { throw Error('duplicate') }), first)
  gate.resolve(map())
  assert.equal((await first).revision, 1)
  assert.equal(state.sessionMapLoads.size, 0)
  await flow.load('codex:b', async () => { throw Object.assign(Error('missing'), { status: 404 }) })
  assert.equal(await flow.load('codex:b', () => { throw Error('duplicate missing read') }), null)
})
test('late loads cannot replace writes, resurrect deleted maps or remove newer flights', async () => {
  const { flow, state } = fixture(), old = deferred(), fresh = deferred()
  const oldLoad = flow.load('codex:a', () => old.promise)
  flow.publish('codex:a', map(2))
  const freshLoad = flow.load('codex:a', () => fresh.promise, true)
  old.resolve(map(1))
  assert.equal((await oldLoad).revision, 2)
  assert.equal(state.sessionMapLoads.get('codex:a'), freshLoad)
  await flow.remove('codex:a', async () => {})
  fresh.resolve(map(3))
  assert.equal(await freshLoad, null)
  assert.equal(flow.current('codex:a'), null)
})
test('out of order mutation responses cannot roll revision backward or replace a recreated map', async () => {
  const { flow } = fixture(), old = deferred()
  flow.publish('codex:a', map())
  const mutation = flow.update('codex:a', () => old.promise)
  await flow.update('codex:a', async () => map(3))
  old.resolve(map(2))
  assert.equal((await mutation).revision, 3)
  const late = deferred(), changing = flow.update('codex:a', () => late.promise)
  flow.publish('codex:a', null)
  flow.publish('codex:a', map(1, 'replacement'))
  late.resolve(map(4))
  assert.equal((await changing).id, 'replacement')
})
test('Map sync is deduplicated and deleted-map results cannot publish status or operations', async () => {
  const { flow, state } = fixture(), gate = deferred()
  flow.publish('codex:a', map())
  let calls = 0, operations = 0
  const task = async (_, isCurrent) => { calls++; await gate.promise; if (isCurrent()) operations++; return flow.current('codex:a') }
  const messages = { syncing: 'working', synced: 'ready' }
  const first = flow.synchronize('codex:a', task, messages)
  assert.equal(flow.synchronize('codex:a', task, messages), first)
  await Promise.resolve()
  await flow.remove('codex:a', async () => {})
  gate.resolve()
  assert.equal(await first, null)
  assert.equal(calls, 1)
  assert.equal(operations, 0)
  assert.equal(state.sessionMapSync.has('codex:a'), false)
})
test('sync failure is isolated to its session and permits retry', async () => {
  const { flow, state } = fixture()
  flow.publish('codex:a', map())
  const messages = { syncing: 'working', synced: 'ready' }
  await assert.rejects(flow.synchronize('codex:a', async () => { throw Error('offline') }, messages), /offline/)
  assert.equal(state.sessionMapSync.get('codex:a').state, 'error')
  assert.equal(state.sessionMapSync.has('codex:b'), false)
  await flow.synchronize('codex:a', async () => map(), messages)
  assert.equal(state.sessionMapSync.get('codex:a').state, 'synced')
})

test('cancelled generation is skipped before execution and stale failures do not affect a replacement', async () => {
  const { flow, state } = fixture(), gate = deferred()
  const messages = { syncing: 'working', synced: 'ready' }
  flow.publish('codex:a', map())
  const skipped = flow.synchronize('codex:a', async () => { assert.fail('cancelled job started') }, messages)
  flow.publish('codex:a', null)
  assert.equal(await skipped, null)
  flow.publish('codex:a', map(1, 'new'))
  const stale = flow.synchronize('codex:a', () => gate.promise, messages)
  await Promise.resolve()
  flow.publish('codex:a', map(1, 'newer'))
  await flow.synchronize('codex:a', async current => current, messages)
  gate.reject(Error('old worker failed'))
  assert.equal((await stale).id, 'newer')
  assert.equal(state.sessionMapSync.get('codex:a').state, 'synced')
})

test('failed or outdated deletions retain the current Map', async () => {
  const { flow } = fixture(), gate = deferred()
  flow.publish('codex:a', map())
  await assert.rejects(flow.remove('codex:a', async () => { throw Error('offline') }), /offline/)
  assert.equal(flow.current('codex:a').id, 'map')
  const deleting = flow.remove('codex:a', () => gate.promise)
  assert.equal(await flow.remove('codex:a', async () => { assert.fail('duplicate deletion') }), false)
  flow.publish('codex:a', map(1, 'replacement'))
  gate.resolve()
  assert.equal(await deleting, false)
  assert.equal(flow.current('codex:a').id, 'replacement')
})
