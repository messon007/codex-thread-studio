import test from 'node:test'
import assert from 'node:assert/strict'
import { runHiddenUtilitySession } from './hidden-utility-session.mjs'

function fixture(codex = true) {
  const state = { hiddenUtilityThreadNames: new Set(), hiddenUtilityThreads: new Set(), hiddenCodexThreads: new Set(), hiddenCodexTurns: new Set(), structuredUtilityTasks: new Map() }
  const calls = []
  const options = {
    backend: codex ? 'ept-codex' : 'opencode', codex, cwd: '/project', model: 'chosen-model', effort: 'low', name: 'utility',
    instructions: 'instructions', input: 'input', outputSchema: { type: 'object' }, validateBeforeStart: true,
    missingTaskMessage: 'missing task', timeoutMessage: 'timeout', ensureCurrent() {},
    parse: () => ({ status: 'completed', prompt: 'next' }), translateError: value => value,
    rpc: async (method, params, timeout) => {
      calls.push([method, params, timeout])
      return method === 'thread/start' ? { thread: { id: 'session' } } : method === 'turn/start' ? { turn: { id: 'turn', status: 'inProgress', items: [] } } : { thread: { turns: [] } }
    },
    remove: async (backend, params, timeout) => { calls.push(['delete', backend, params, timeout]) },
    cleanupError: error => calls.push(['cleanup-error', error.message]),
    sessionKey: (backend, id) => `${backend}:${id}`, turnKey: (backend, id) => `${backend}:${id}`,
  }
  return { state, calls, options }
}

test('hidden Codex task preserves model/schema and reads notifications, not remote history', async () => {
  const { state, calls, options } = fixture()
  const rpc = options.rpc
  options.rpc = async (...args) => {
    if (args[0] === 'turn/start') {
      // A completion notification can beat the turn/start acknowledgement.
      state.structuredUtilityTasks.get('ept-codex:session').model.turns.push({ id: 'turn', status: 'completed', items: [] })
    }
    return rpc(...args)
  }
  options.parse = model => {
    assert.equal(model.turns.length, 1)
    assert.equal(model.turns[0].status, 'completed')
    return { status: 'completed', prompt: 'next' }
  }
  assert.equal((await runHiddenUtilitySession(state, options)).prompt, 'next')
  await Promise.resolve()
  assert.deepEqual(calls.map(call => call[0]), ['thread/start', 'turn/start', 'delete'])
  assert.equal(calls[0][1].ephemeral, true)
  assert.equal(calls[0][1].model, 'chosen-model')
  assert.equal(calls[1][1].effort, 'low')
  assert.deepEqual(calls[1][1].outputSchema, { type: 'object' })
  assert.deepEqual(calls[2], ['delete', 'ept-codex', { threadId: 'session' }, 15000])
  for (const collection of Object.values(state)) assert.equal(collection.size, 0)
})

test('hidden OpenCode task uses history and keeps directory on deletion', async () => {
  const { state, calls, options } = fixture(false)
  await runHiddenUtilitySession(state, options)
  assert.deepEqual(calls.map(call => call[0]), ['thread/start', 'turn/start', 'thread/read', 'delete'])
  assert.equal(calls[0][1].name, 'utility')
  assert.equal(calls[0][1].ephemeral, undefined)
  assert.equal(calls[1][1].developerInstructions, 'instructions')
  assert.deepEqual(calls[3], ['delete', 'opencode', { threadId: 'session', cwd: '/project' }, 15000])
})

test('failed creation clears name without deleting an unknown session', async () => {
  const { state, calls, options } = fixture()
  options.rpc = async () => ({})
  await assert.rejects(runHiddenUtilitySession(state, options), /missing task/)
  assert.equal(calls.length, 0)
  assert.equal(state.hiddenUtilityThreadNames.size, 0)
})

test('selection change after creation cleans the original backend without sending a turn', async () => {
  const { state, calls, options } = fixture()
  let checks = 0
  options.ensureCurrent = () => { if (++checks > 1) throw new Error('changed') }
  await assert.rejects(runHiddenUtilitySession(state, options), /changed/)
  await Promise.resolve()
  assert.deepEqual(calls.map(call => call[0]), ['thread/start', 'delete'])
  assert.equal(state.hiddenUtilityThreads.size, 0)
})

test('deletion remains asynchronous and failures do not replace a successful task result', async () => {
  const { state, calls, options } = fixture()
  let rejectDelete
  options.remove = () => new Promise((_, reject) => { rejectDelete = reject })
  assert.equal((await runHiddenUtilitySession(state, options)).prompt, 'next')
  assert.equal(state.structuredUtilityTasks.size, 0)
  assert.equal(state.hiddenUtilityThreads.size, 1)
  rejectDelete(new Error('delete failed'))
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(state.hiddenUtilityThreads.size, 0)
  assert.deepEqual(calls.at(-1), ['cleanup-error', 'delete failed'])
})

test('turn rejection and parsed backend failure both clean registered tasks', async () => {
  for (const rejectTurn of [true, false]) {
    const { state, options } = fixture()
    const rpc = options.rpc
    options.rpc = async (...args) => {
      if (rejectTurn && args[0] === 'turn/start') throw new Error('turn rejected')
      return rpc(...args)
    }
    options.parse = () => ({ status: 'failed', error: 'backend failed' })
    await assert.rejects(runHiddenUtilitySession(state, options), rejectTurn ? /turn rejected/ : /backend failed/)
    await Promise.resolve()
    for (const collection of Object.values(state)) assert.equal(collection.size, 0)
  }
})
