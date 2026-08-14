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

test('rejects malformed references and incomplete adapters', () => {
  const registry = new SessionDispatchRegistry()
  assert.throws(() => registry.register('Bad Backend', {}), /Invalid session backend/)
  assert.throws(() => registry.register('valid', { read() {} }), /read and startTurn/)
  assert.throws(() => normalizeSessionRef({ backend: 'codex', id: '' }), /Invalid session reference/)
  assert.throws(() => registry.read({ backend: 'missing', id: 'one' }), /Unsupported session backend/)
})
