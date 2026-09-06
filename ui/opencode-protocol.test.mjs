import assert from 'node:assert/strict'
import test from 'node:test'
import { createOpenCodeProtocol } from './opencode-protocol.mjs'

function fixture(handle) {
  const requests = [], sequences = new Map()
  const state = { backend: 'opencode', ready: true, selectedId: 'a', threads: [{ id: 'a', cwd: '/work' }], threadsByBackend: { opencode: [{ id: 'a', cwd: '/work' }] }, hiddenSessionDirectories: [], sessionDirectoryIgnore: [] }
  const protocol = createOpenCodeProtocol(state, {
    gatewayFetch: async (path, options) => { requests.push({ path, ...options }); return handle(path, options) },
    selectedThread: () => state.threads[0], currentTurnOptions: () => ({}), t: s => s,
    studioPerformance: { start: () => () => {} }, threadCatalogKey: (b, id) => `${b}:${id}`, openCodeHistoryEventSequences: sequences,
  })
  return { protocol, requests, state, sequences }
}
const json = (value, options = {}) => new Response(JSON.stringify(value), options)

test('OpenCode send reads the server ID without injecting a client message ID', async () => {
  let accepted = false
  const f = fixture((path, request) => {
    if (path.includes('prompt_async')) { accepted = true; return new Response(null, { status: 204 }) }
    return json(accepted ? [{ info: { id: 'msg_server', role: 'user' }, parts: [{ type: 'text', text: 'hello' }] }] : [])
  })
  const result = await f.protocol.openCodeRpc('turn/start', { threadId: 'a', input: [{ type: 'text', text: 'hello' }], model: 'provider/model' })
  assert.equal(result.turn.id, 'msg_server')
  const sent = JSON.parse(f.requests.find(r => r.path.includes('prompt_async')).body)
  assert.deepEqual(sent.model, { providerID: 'provider', modelID: 'model' })
  assert.equal(sent.messageID, undefined)
  assert.deepEqual(sent.parts, [{ type: 'text', text: 'hello' }])
})

test('inactive access is explicit and read-back survives a backend switch after acceptance', async () => {
  let f, accepted = false
  f = fixture(path => {
    if (path.includes('prompt_async')) { accepted = true; f.state.backend = 'codex'; return new Response(null, { status: 204 }) }
    return json(accepted ? [{ info: { id: 'server', role: 'user' }, parts: [] }] : [])
  })
  assert.equal((await f.protocol.openCodeRpc('turn/start', { threadId: 'a', input: [] })).turn.id, 'server')
  await assert.rejects(f.protocol.openCodeFetch('/session'), /not ready/)
})

test('message history preserves pagination cursor and authoritative status sequence', async () => {
  const f = fixture(path => path.includes('/session/status') ? json({ a: { type: 'idle' } })
    : json([{ info: { id: 'u', role: 'user' }, parts: [] }]))
  f.sequences.set('a', 3)
  const history = await f.protocol.fetchOpenCodeMessageHistory('a', '/work', {})
  assert.equal(history.messages.length, 1)
  assert.equal(history.complete, true)
  assert.deepEqual(history.historyMessageSnapshots.u, { afterSequence: 3, ambiguousThroughSequence: 3 })
  assert.equal((await f.protocol.fetchOpenCodeStatusSnapshot('a', '/work', {})).statusEventSequence, 3)
})

test('HTTP failures and aborts retain useful protocol errors', async () => {
  const f = fixture(() => json({ error: { message: 'rejected' } }, { status: 400 }))
  await assert.rejects(f.protocol.openCodeFetch('/session'), /rejected/)
  const abort = fixture(() => { throw new DOMException('aborted', 'AbortError') })
  await assert.rejects(abort.protocol.openCodeFetch('/session'), /request timed out/)
})
