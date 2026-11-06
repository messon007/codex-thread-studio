import assert from 'node:assert/strict'
import { searchController } from '../../web/search-controller.mjs'
const deferred = () => { let resolve, reject; const promise = new Promise((done, fail) => { resolve = done; reject = fail }); return { promise, resolve, reject } }
let owner = { documentId: 'pane-one', revision: 'revision-one', source: true, renderer: false, editing: false }
let calls = [], activated = [], notifications = [], painted = [], pending = null, activation = null, cancelled = 0
const values = Array.from({ length: 250 }, (_, index) => ({ start: index * 10, end: index * 10 + 6, line: 1, page: 0, snippet: 'needle', occurrence: index }))
const controller = searchController({
  owner: () => owner, describe: match => ({ ...match }),
  async load(query, offset, expected) { calls.push({ query, offset }); if (pending) await pending.promise; return { revision: expected.revision, offset, total: values.length, matches: values.slice(offset, offset + 100) } },
  async activate(match, options) { activated.push({ match, options }); if (activation) await activation.promise },
  changed: state => notifications.push(state), cancel: () => cancelled++, clear() {}, paint: (query, active) => painted.push({ query, active }),
})
let result = await controller.search({ query: 'needle', offset: 100 })
assert.equal(result.matches.length, 100); assert.equal(result.total, 250); assert.equal(result.totalExact, true)
assert.equal(result.loading, false); assert.equal(result.current, null); assert.equal(result.matches[0].index, 100)
const id = result.searchId
await controller.activate({ searchId: id, index: 149 })
assert.equal(controller.state().current, 149); assert.equal(activated.at(-1).match.start, 1490)
await controller.activate({ searchId: id, index: 249 })
assert.equal(controller.state().current, 249); assert.equal(controller.state().offset, 200)
assert.equal(activated.at(-1).match.occurrence, 249)
assert.deepEqual(calls.map(call => call.offset), [100, 200])
result = await controller.search({ query: 'needle', offset: 0 })
assert.equal(result.searchId, id); assert.equal(result.active.index, 249)
for (const offset of [-1, 1.5, null, '1', Infinity]) await assert.rejects(controller.search({ query: 'needle', offset }), { code: 'PARAMS' })
await assert.rejects(controller.search({ query: 'needle', offset: 251 }), { code: 'RANGE' })
await assert.rejects(controller.activate({ searchId: id, index: 250 }), { code: 'RANGE' })
await controller.search({ query: 'different' })
await assert.rejects(controller.activate({ searchId: id, index: 0 }), { code: 'SEARCH_ID' })
// A provider that ignores AbortSignal must still be unable to publish late data.
pending = deferred()
const abort = new AbortController()
const late = controller.search({ query: 'cancelled' }, abort.signal).catch(error => error)
abort.abort(); controller.clear(); pending.resolve(); pending = null
assert.equal((await late).name, 'AbortError'); assert.equal(controller.state().searchId, null)
pending = deferred()
const older = controller.search({ query: 'older' }).catch(error => error)
const oldPending = pending; pending = null
const newer = await controller.search({ query: 'newer' })
oldPending.resolve(); assert.equal((await older).name, 'AbortError')
assert.equal(controller.state().searchId, newer.searchId)
pending = deferred()
const rejectedOld = controller.search({ query: 'rejected-old' }).catch(error => error)
const rejectedPending = pending; pending = null
await controller.search({ query: 'newer' })
rejectedPending.reject(Error('Late failed provider'))
assert.equal((await rejectedOld).code, 'CANCELLED')
const activeIdentity = controller.state().searchId

activation = deferred()
const activeAbort = new AbortController()
const delayedActivation = controller.activate({ searchId: activeIdentity, index: 3 }, activeAbort.signal).catch(error => error)
activeAbort.abort(); activation.resolve(); activation = null
assert.equal((await delayedActivation).name, 'AbortError'); assert.equal(controller.state().active, null)
await controller.activate({ searchId: activeIdentity, index: 4 })
assert.equal(controller.state().current, 4)
activation = deferred()
const racingActivation = controller.activate({ searchId: activeIdentity, index: 5 })
const paintsDuringActivation = painted.length
controller.repaint()
assert.equal(painted.length, paintsDuringActivation, 'old view readiness cannot clear in-flight highlights')
activation.resolve(); activation = null; await racingActivation
assert.equal(painted.at(-1).active.index, 5)

pending = deferred()
const oldRevision = controller.search({ query: 'revision' }).catch(error => error)
owner = { ...owner, revision: 'revision-two' }; pending.resolve(); pending = null
assert.equal((await oldRevision).code, 'REVISION')
controller.dispose(); await assert.rejects(controller.search({ query: 'needle' }), { code: 'DISPOSED' })
assert.ok(cancelled > 0); assert.ok(notifications.length > 0)
let rendererLoads = 0
const renderer = searchController({
  owner: () => ({ ...owner, renderer: true }), describe: match => match,
  async load() { rendererLoads++; return { matches: Array.from({ length: 500 }, (_, index) => ({ searchMatchId: String(index), location: { kind: 'pdf', page: index + 1 } })), complete: false } },
  activate() {}, paint() {}, clear() {},
})
const capped = await renderer.search({ query: 'needle', offset: 400 })
assert.equal(capped.total, 500); assert.equal(capped.totalExact, false); assert.equal(capped.complete, false)
assert.equal(capped.matches.length, 100); assert.equal(capped.matches[0].index, 400)
await renderer.activate({ searchId: capped.searchId, index: 1 })
assert.equal(rendererLoads, 1)
await assert.rejects(renderer.search({ query: 'needle', offset: 501 }), { code: 'RANGE' })
assert.equal(rendererLoads, 1)
assert.equal((await renderer.search({ query: '' })).searchId, null)
console.log(JSON.stringify({ ok: true, checks: ['source-paging', 'identity', 'verified-activation', 'same-line-occurrence', 'parameter-rejection', 'abort-late-fetch', 'abort-late-error', 'supersession', 'activation-cancel-retry', 'revision', 'disposal', 'capped-renderer-paging'] }))
