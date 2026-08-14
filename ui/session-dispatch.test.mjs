import test from 'node:test'
import assert from 'node:assert/strict'
import { SessionDispatchRegistry, normalizeSessionRef } from './session-dispatch.mjs'

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

test('rejects malformed references and incomplete adapters', () => {
  const registry = new SessionDispatchRegistry()
  assert.throws(() => registry.register('Bad Backend', {}), /Invalid session backend/)
  assert.throws(() => registry.register('valid', { read() {} }), /read and startTurn/)
  assert.throws(() => normalizeSessionRef({ backend: 'codex', id: '' }), /Invalid session reference/)
  assert.throws(() => registry.read({ backend: 'missing', id: 'one' }), /Unsupported session backend/)
})
