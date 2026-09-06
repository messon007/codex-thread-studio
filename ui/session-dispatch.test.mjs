import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import {
  isThreadNotFoundError,
  SessionDispatchRegistry,
  normalizeSessionRef,
  startTurnWithPreparation,
} from './session-dispatch.mjs'

const appSource = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('dispatches by session backend without embedding backend policy', async () => {
  const calls = []
  const registry = new SessionDispatchRegistry()
    .register('codex', {
      read: (ref) => ({ ref }),
      prepareTurn: (ref) => calls.push({ operation: 'prepare', ref }),
      startTurn: (ref, input) => calls.push({ ref, input }),
    })
    .register('third-party', {
      read: (ref) => ({ ref }),
      startTurn: (ref, input) => calls.push({ ref, input }),
    })

  assert.deepEqual(registry.backends(), ['codex', 'third-party'])
  assert.equal(registry.supports('third-party'), true)
  assert.deepEqual((await registry.read({ backend: 'third-party', id: 'one' })).ref, {
    backend: 'third-party', id: 'one', key: 'third-party:one',
  })
  await registry.prepareTurn({ backend: 'codex', id: 'two' })
  await registry.startTurn({ backend: 'codex', id: 'two' }, 'hello')
  assert.deepEqual(calls.map((call) => call.operation || 'start'), ['prepare', 'start'])
  assert.equal(calls[1].ref.key, 'codex:two')
  assert.equal(await registry.prepareTurn({ backend: 'third-party', id: 'one' }), null)
})

test('prepares an inactive session once per backend lifecycle', async () => {
  let preparations = 0
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: async () => { preparations += 1 },
    startTurn: () => null,
  })

  await Promise.all([
    registry.prepareTurn({ backend: 'codex', id: 'cold' }),
    registry.prepareTurn({ backend: 'codex', id: 'cold' }),
  ])
  await registry.prepareTurn({ backend: 'codex', id: 'cold' })
  await registry.prepareTurn({ backend: 'codex', id: 'warm' }, { alreadyActive: true })
  assert.equal(preparations, 1)

  registry.clearPrepared('codex')
  await registry.prepareTurn({ backend: 'codex', id: 'cold' })
  assert.equal(preparations, 2)
})

test('does not retain a preparation that finishes after its backend was reset', async () => {
  let release
  let preparations = 0
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: () => {
      preparations += 1
      return preparations === 1 ? new Promise((resolve) => { release = resolve }) : null
    },
    startTurn: () => null,
  })

  const stalePreparation = registry.prepareTurn({ backend: 'codex', id: 'target' })
  registry.clearPrepared('codex')
  release()
  await stalePreparation
  await registry.prepareTurn({ backend: 'codex', id: 'target' })
  assert.equal(preparations, 2)
})

test('clearing one prepared session does not add work to other sessions', async () => {
  const prepared = []
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: async (ref) => { prepared.push(ref.key) },
    startTurn: () => null,
  })

  await registry.prepareTurn({ backend: 'codex', id: 'one' })
  await registry.prepareTurn({ backend: 'codex', id: 'two' })
  registry.clearPreparedSession({ backend: 'codex', id: 'one' })
  await registry.prepareTurn({ backend: 'codex', id: 'one' })
  await registry.prepareTurn({ backend: 'codex', id: 'two' })

  assert.deepEqual(prepared, ['codex:one', 'codex:two', 'codex:one'])
})

test('a stale preparation cannot re-prepare a session after session-only clearing', async () => {
  let release
  let preparations = 0
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: () => {
      preparations += 1
      return preparations === 1 ? new Promise((resolve) => { release = resolve }) : null
    },
    startTurn: () => null,
  })

  const stale = registry.prepareTurn({ backend: 'codex', id: 'one' })
  registry.clearPreparedSession({ backend: 'codex', id: 'one' })
  release()
  await stale
  await registry.prepareTurn({ backend: 'codex', id: 'one' })
  assert.equal(preparations, 2)
})

test('prepared turn retries thread-not-found once without repeating normal preparation RPCs', async () => {
  let preparations = 0
  let starts = 0
  const ref = { backend: 'codex', id: 'one' }
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: async () => { preparations += 1 },
    startTurn: () => null,
  })
  const prepare = () => registry.prepareTurn(ref)
  const start = async () => {
    starts += 1
    if (starts === 1) throw new Error('thread one not found')
    return 'started'
  }

  assert.equal(await startTurnWithPreparation({
    registry, ref, prepare, start, recoverThreadNotFound: true,
  }), 'started')
  assert.equal(preparations, 2)
  assert.equal(starts, 2)

  await startTurnWithPreparation({ registry, ref, prepare, start, recoverThreadNotFound: true })
  assert.equal(preparations, 2)
  assert.equal(starts, 3)
})

test('prepared turn preserves preparation errors and does not retry unrelated start errors', async () => {
  const ref = { backend: 'codex', id: 'one' }
  const registry = new SessionDispatchRegistry().register('codex', {
    read: () => null,
    prepareTurn: () => null,
    startTurn: () => null,
  })
  let starts = 0
  await assert.rejects(startTurnWithPreparation({
    registry,
    ref,
    prepare: async () => { throw new Error('thread-store conflict: already has an active writer') },
    start: async () => { starts += 1 },
    recoverThreadNotFound: true,
  }), /active writer/u)
  assert.equal(starts, 0)

  await assert.rejects(startTurnWithPreparation({
    registry,
    ref,
    prepare: async () => {},
    start: async () => { starts += 1; throw new Error('connection closed') },
    recoverThreadNotFound: true,
  }), /connection closed/u)
  assert.equal(starts, 1)
  assert.equal(isThreadNotFoundError(new Error('favorite not found')), false)
})

test('the normal Composer integrates preparation without duplicating Session Map resumes', () => {
  assert.match(appSource, /async function prepareComposerTurn\(ref\)[\s\S]*sessionMap\.prepareTurn\(ref\)[\s\S]*sessionDispatch\.markPrepared\(ref\)[\s\S]*sessionDispatch\.prepareTurn\(ref\)/u)
  assert.match(appSource, /startTurnWithPreparation\(\{[\s\S]*prepare: \(\) => prepareComposerTurn\(ref\)[\s\S]*recoverThreadNotFound: isCodexBackend\(backend\)/u)
})

test('rejects malformed references and incomplete adapters', () => {
  const registry = new SessionDispatchRegistry()
  assert.throws(() => registry.register('Bad Backend', {}), /Invalid session backend/)
  assert.throws(() => registry.register('valid', { read() {} }), /read and startTurn/)
  assert.throws(() => normalizeSessionRef({ backend: 'codex', id: '' }), /Invalid session reference/)
  assert.throws(() => registry.read({ backend: 'missing', id: 'one' }), /Unsupported session backend/)
})
