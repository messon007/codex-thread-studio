import assert from 'node:assert/strict'
import test from 'node:test'
import { createLargeMarkdownHost } from './large-markdown-host.mjs'
import { searchController } from '../crates/marknoria/web/search-controller.mjs'

function fixture({ renderer = false } = {}) {
  let view = 'preview', revision = 'a'.repeat(64), listener, loadGate, activateGate
  const calls = [], painted = [], states = []
  const owner = () => ({ documentId: 'owned', revision, source: !renderer, renderer, editing: view === 'edit' })
  const state = () => ({ ...owner(), state: 'ready', view, search: controller.state(), capabilities: { search: controller.capabilities() } })
  const controller = searchController({
    owner, describe: value => value,
    async load(query, offset, expected, signal, options) {
      calls.push({ query, offset, matchCase: options.matchCase })
      const gate = loadGate; loadGate = null
      if (gate) await gate
      const matches = Array.from({ length: renderer ? 500 : 250 }, (_, index) => ({
        line: 1, start: index * 10, end: index * 10 + 6, occurrence: index,
        location: renderer ? { kind: 'pdf', page: index + 1 } : { kind: 'line', line: 1 },
        locationPrecision: renderer ? 'page' : 'line',
      }))
      return renderer ? { matches, complete: false } : { revision: expected.revision, offset, total: matches.length, matches: matches.slice(offset, offset + 100) }
    },
    async activate(match) { if (activateGate) { const gate = activateGate; activateGate = null; await gate } },
    changed() { listener?.('state', state()) },
    paint(query, active) { painted.push({ query, active }) }, clear() { painted.push({ query: '', active: null }) },
  })
  const requests = []
  const client = {
    ready: async () => state(),
    async request(method, params, { signal } = {}) {
      requests.push({ method, params })
      if (params?.documentId && (params.documentId !== owner().documentId || params.revision !== revision)) throw Object.assign(Error('Stale owner'), { code: 'REVISION' })
      if (method === 'search') return controller.search(params, signal)
      if (method === 'searchActivate') await controller.activate(params, signal)
      if (method === 'searchClear') controller.clear()
      return state()
    },
    async dispose() { controller.dispose() }, disconnect() {},
  }
  const host = createLargeMarkdownHost({ src: 'http://localhost/reader/#token' }, value => states.push(value), () => {}, {
    createClient(_frame, options) { listener = options.onEvent; return client },
  })
  return { host, states, controller, calls, requests, painted,
    deferLoad() { return new Promise(resolve => { loadGate = new Promise(done => resolve(done)) }) },
    deferActivation() { return new Promise(resolve => { activateGate = new Promise(done => resolve(done)) }) },
    replace() { revision = 'b'.repeat(64); controller.clear(); listener('state', state()) },
  }
}

test('host activates absolute source occurrences beyond one bounded window and exposes exact totals independently of complete', async () => {
  const f = fixture()
  try {
    const first = await f.host.search('中文😀')
    assert.equal(first.total, 250); assert.equal(first.totalExact, true); assert.equal(first.complete, false)
    assert.equal(first.matches.length, 100)
    const last = await f.host.activateSearch(first, 249)
    assert.equal(last.search.current, 249); assert.equal(last.search.offset, 200)
    assert.equal(last.search.active.occurrence, 249)
    assert.deepEqual(f.calls.map(value => value.offset), [0, 200])
    assert.equal(f.states.at(-1).search.total, 250)
    assert.equal('matches' in f.states.at(-1).search, false)
    assert.deepEqual(f.requests.at(-1).params, { searchId: first.searchId, index: 249, documentId: first.documentId, revision: first.revision })
    const second = await f.host.search('中文😀', 100)
    assert.equal(second.searchId, first.searchId); assert.equal(second.matches[0].index, 100)
  } finally { await f.host.dispose() }
})

test('renderer lower bounds retain page precision and only activate the declared available results', async () => {
  const f = fixture({ renderer: true })
  try {
    const result = await f.host.search('needle', 400)
    assert.equal(result.total, 500); assert.equal(result.totalExact, false); assert.equal(result.complete, false)
    const active = await f.host.activateSearch(result, 499)
    assert.equal(active.search.active.locationPrecision, 'page')
    await assert.rejects(f.host.activateSearch(result, 500), { code: 'RANGE' })
    assert.equal(f.calls.length, 1)
  } finally { await f.host.dispose() }
})

test('superseded search and activation cannot republish stale state after clear or replacement', async () => {
  const f = fixture()
  try {
    await f.host.ready
    const releaseLoad = await f.deferLoad()
    const old = f.host.search('old').catch(error => error)
    await new Promise(resolve => setImmediate(resolve))
    await f.host.clearSearch()
    const latest = await f.host.search('   ')
    releaseLoad()
    assert.equal((await old).name, 'AbortError')
    assert.equal(f.states.at(-1).search.query, '   ', 'literal whitespace query is retained')
    const releaseActivation = await f.deferActivation()
    const older = f.host.activateSearch(latest, 101).catch(error => error)
    await new Promise(resolve => setImmediate(resolve))
    await f.host.activateSearch(latest, 249)
    releaseActivation()
    assert.equal((await older).name, 'AbortError')
    assert.equal(f.states.at(-1).search.current, 249)
    await f.host.clearSearch()
    assert.equal(f.states.at(-1).search.searchId, null)
    await assert.rejects(f.host.activateSearch(latest, 0), { code: 'SEARCH_ID' })
    f.replace()
    await assert.rejects(f.host.activateSearch(latest, 0), { code: 'REVISION' })
  } finally { await f.host.dispose() }
})


test('explicit case mode survives bounded paging and cancels an old-mode identity', async () => {
  const f = fixture()
  try {
    const insensitive = await f.host.search('K', 0, false)
    await f.host.activateSearch(insensitive, 101)
    assert.equal(f.calls.at(-1).matchCase, false)
    const strict = await f.host.search('K', 0, true)
    assert.notEqual(strict.searchId, insensitive.searchId)
    assert.equal(strict.matchCase, true)
    await assert.rejects(f.host.activateSearch(insensitive, 0), { code: 'SEARCH_ID' })
    const gate = await f.deferLoad()
    const old = f.host.search('other', 0, false).catch(error => error)
    await new Promise(resolve => setImmediate(resolve))
    const latest = await f.host.search('other', 0, true)
    gate(); assert.equal((await old).name, 'AbortError')
    assert.equal(latest.matchCase, true); assert.equal(f.states.at(-1).search.matchCase, true)
  } finally { await f.host.dispose() }
})
