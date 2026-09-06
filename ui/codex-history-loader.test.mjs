import test from 'node:test'
import assert from 'node:assert/strict'
import { createCodexHistoryLoader } from './codex-history-loader.mjs'
const turn = id => ({ id, status: 'completed', items: [] })
const cached = { model: { threadId: 'one', turns: [turn('old'), turn('anchor')] } }
function fixture(overrides = {}) {
  const calls = []
  const services = {
    historyTailCapability: () => undefined,
    rememberHistoryTailCapability: (...args) => calls.push(['capability', ...args]),
    threadForRef: () => ({ name: 'catalog' }),
    requestCodexResume: async (...args) => { calls.push(['resume', ...args]); return { thread: { id: 'one' }, initialTurnsPage: { data: [turn('new'), turn('anchor')] } } },
    dispatchBackendRpc: async (...args) => { calls.push(['rpc', ...args]); return { thread: { id: 'one', turns: [turn('full')] } } },
    ...overrides,
  }
  return { ...createCodexHistoryLoader(services), calls }
}
test('selection uses initial tail without an extra list/read and forwards environment settings', async () => {
  const loader = fixture()
  const result = await loader.loadCodexHistoryForSelection('ept-codex', 'one', cached, { environmentRoot: '/work', environmentRevision: 'rev' })
  assert.equal(result.historyMode, 'tail')
  assert.deepEqual(result.result.thread.turns.map(t => t.id), ['old', 'anchor', 'new'])
  assert.deepEqual(loader.calls[0], ['resume', 'ept-codex', 'one', { environmentRoot: '/work', environmentRevision: 'rev', incremental: true }])
  assert.equal(loader.calls.filter(c => c[0] === 'rpc').length, 0)
})
test('cold and unsupported selections resume once; legacy full responses are not read again', async () => {
  for (const state of [null, cached]) {
    const loader = fixture({ historyTailCapability: () => false })
    const result = await loader.loadCodexHistoryForSelection('codex', 'one', state)
    assert.equal(result.historyMode, 'full')
    assert.equal(loader.calls.length, 1)
  }
  const loader = fixture({ requestCodexResume: async () => ({ thread: { turns: [turn('legacy')] } }) })
  assert.equal((await loader.loadCodexHistoryForSelection('codex', 'one', cached)).historyMode, 'legacy-full')
  assert.deepEqual(loader.calls, [['capability', 'codex', 'one', false]])
})
test('an unmatched tail falls back to full history without repeating resume', async () => {
  const loader = fixture({ requestCodexResume: async () => ({ thread: { name: 'resume metadata' }, initialTurnsPage: { data: [turn('unrelated')] } }) })
  const result = await loader.loadCodexHistoryForSelection('codex', 'one', cached)
  assert.equal(result.historyMode, 'full-fallback')
  assert.equal(result.result.thread.name, 'resume metadata')
  assert.deepEqual(result.result.thread.turns.map(t => t.id), ['full'])
  assert.equal(loader.calls.filter(c => c[0] === 'rpc').length, 1)
})
test('background tail loading never resumes and propagates non-compatibility errors', async () => {
  const loader = fixture({ dispatchBackendRpc: async (_backend, method) => {
    assert.equal(method, 'thread/turns/list')
    return { data: [turn('new'), turn('anchor')] }
  } })
  const result = await loader.loadCodexHistoryForBackground('codex', 'one', cached)
  assert.equal(result.historyMode, 'tail')
  assert.equal(result.result.thread.name, 'catalog')
  assert.equal(loader.calls.some(c => c[0] === 'resume'), false)
  const error = new Error('connection lost')
  const broken = fixture({ dispatchBackendRpc: async () => { throw error } })
  await assert.rejects(broken.loadCodexHistoryForBackground('codex', 'one', cached), value => value === error)
})

test('unsupported pagination falls back once, without masking unrelated resume failures', async () => {
  let resumes = 0
  const loader = fixture({ requestCodexResume: async () => {
    resumes++
    if (resumes === 1) throw new Error('Invalid params: unknown field excludeTurns')
    return { thread: { id: 'one', turns: [turn('full')] } }
  } })
  const result = await loader.loadCodexHistoryForSelection('codex', 'one', cached)
  assert.equal(result.historyMode, 'compatibility-fallback')
  assert.equal(resumes, 2)
  const error = new Error('thread-store conflict: active writer')
  const failed = fixture({ requestCodexResume: async () => { throw error } })
  await assert.rejects(failed.loadCodexHistoryForSelection('codex', 'one', cached), value => value === error)
  assert.deepEqual(failed.calls, [])
})
