import test from 'node:test'
import assert from 'node:assert/strict'
import { installSelectedHistory, hydrateOpenCodeHistoryMetadata } from './history-installation.mjs'
import { createCodexViewModel, hydrateCodexThread } from './codex-native.mjs'
import { mergeOpenCodeThreadTail, replayOpenCodeEventsAfterHistory } from './opencode-native.mjs'

function fixture(overrides = {}) {
  const calls = [], model = { historyComplete: false }
  return { calls, model, options: {
    opencode: true, model, thread: { status: 'idle' }, historyAnchorTurnId: 'anchor', historyComplete: true,
    historyEvents: ['event'], messageSnapshots: { message: { afterSequence: 5 } }, statusSequence: 7,
    mergeTail: () => { calls.push('tail'); return false }, hydrate: () => calls.push('hydrate'),
    hydrateMetadata: () => calls.push('metadata'), replay: (_model, events, options) => calls.push(['replay', events, options]),
    ready: value => calls.push(['ready', value.complete]), mergeMetadata: () => calls.push('catalog'), cache: () => calls.push('cache'),
    ...overrides,
  } }
}

test('failed tail merge hydrates before metadata/replay, then marks ready and caches', () => {
  const f = fixture()
  installSelectedHistory(f.options)
  assert.deepEqual(f.calls, ['tail', 'hydrate', 'metadata', ['replay', ['event'], {
    messageSnapshots: { message: { afterSequence: 5 } }, statusAfterSequence: 7, authoritativeStatus: 'idle',
  }], ['ready', true], 'catalog', 'cache'])
})

test('successful tail merge preserves earlier history completeness and skips full hydration', () => {
  const f = fixture({ mergeTail: () => true })
  installSelectedHistory(f.options)
  assert.equal(f.calls.includes('hydrate'), false)
  assert.equal(f.calls.includes('metadata'), false)
  assert.deepEqual(f.calls.at(-3), ['ready', false])
})

test('Codex uses full hydration without OpenCode tail/metadata or empty replay', () => {
  const f = fixture({ opencode: false, historyEvents: [] })
  installSelectedHistory(f.options)
  assert.deepEqual(f.calls, ['hydrate', ['ready', true], 'catalog', 'cache'])
})

test('unknown status snapshot does not override replay state', () => {
  for (const statusSequence of [-1, undefined]) {
    const f = fixture({ statusSequence })
    installSelectedHistory(f.options)
    assert.equal(f.calls.find(call => Array.isArray(call) && call[0] === 'replay')[2].authoritativeStatus, null)
  }
})

test('failed replay cannot mark history ready or cache a partial result', () => {
  const f = fixture({ replay: () => { throw new Error('invalid event') } })
  assert.throws(() => installSelectedHistory(f.options), /invalid event/)
  assert.deepEqual(f.calls, ['tail', 'hydrate', 'metadata'])
})

test('OpenCode metadata resets stale maps and derives active turn from authoritative status', () => {
  const model = { turns: [{ id: 'last' }], status: 'running', activeTurnId: 'old', messageTurns: { old: 'old' } }
  hydrateOpenCodeHistoryMetadata({ status: 'idle' }, model, false)
  assert.equal(model.activeTurnId, null)
  assert.deepEqual(model.messageTurns, {})
  assert.equal(model.historyComplete, false)
  hydrateOpenCodeHistoryMetadata({ status: 'running', messageTurns: { user: 'last' } }, model)
  assert.equal(model.activeTurnId, 'last')
  assert.equal(model.historyComplete, true)
  assert.deepEqual(model.messageTurns, { user: 'last' })
})

test('real reducers replay completion received during history loading before caching', () => {
  const model = createCodexViewModel()
  let cached = false
  installSelectedHistory({
    opencode: true, model,
    thread: { id: 'session', status: 'running', turns: [{ id: 'turn', status: 'inProgress', items: [] }] },
    historyEvents: [{ event: { type: 'session.idle', properties: { sessionID: 'session' } }, sequence: 2 }],
    statusSequence: -1,
    mergeTail: mergeOpenCodeThreadTail, hydrate: hydrateCodexThread, hydrateMetadata: hydrateOpenCodeHistoryMetadata,
    replay: (target, events, options) => replayOpenCodeEventsAfterHistory(target, events, 'session', options),
    ready() {}, mergeMetadata() {}, cache: target => {
      assert.equal(target, model)
      assert.equal(target.status, 'idle')
      assert.equal(target.activeTurnId, null)
      assert.equal(target.turns[0].status, 'completed')
      cached = true
    },
  })
  assert.equal(cached, true)
})
