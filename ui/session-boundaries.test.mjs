import test from 'node:test'
import assert from 'node:assert/strict'
import { SessionDispatchRegistry, normalizeSessionRef } from './session-dispatch.mjs'
import { installBackendRegistry, BACKEND_IDS, backendDescriptor } from './backends.mjs'

test('same thread IDs across Codex, EPT and OpenCode keep preparation lifecycles separate', async () => {
  const calls = []
  const registry = new SessionDispatchRegistry()
  let release
  for (const backend of ['codex', 'ept-codex', 'opencode']) {
    registry.register(backend, {
      read: (ref) => ref.key,
      startTurn: (ref) => ref.key,
      ...(backend === 'opencode' ? {} : {
        prepareTurn: (ref) => {
          calls.push(ref.key)
          return backend === 'codex' && calls.filter((key) => key === ref.key).length === 1
            ? new Promise((resolve) => { release = resolve }) : null
        },
      }),
    })
  }
  const ref = (backend) => ({ backend, id: 'same' })
  const old = registry.prepareTurn(ref('codex'))
  assert.equal(registry.prepareTurn(ref('codex')), old, 'concurrent preparations share one promise')
  await registry.prepareTurn(ref('ept-codex'))
  assert.equal(await registry.prepareTurn(ref('opencode')), null)
  registry.clearPrepared('codex')
  const current = registry.prepareTurn(ref('codex'))
  release('old')
  await old
  await current
  await registry.prepareTurn(ref('ept-codex'))
  await registry.prepareTurn(ref('codex'))
  assert.deepEqual(calls, ['codex:same', 'ept-codex:same', 'codex:same'])
  for (const backend of registry.backends()) {
    assert.equal(registry.read(ref(backend)), `${backend}:same`)
    assert.equal(registry.startTurn(ref(backend), 'hello'), `${backend}:same`)
  }
})

test('unknown boundary data is rejected or normalized without trusting TypeScript types', () => {
  for (const value of [null, undefined, 42, 'codex:one', [], {}, { backend: 'codex', id: 42 }]) {
    assert.throws(() => normalizeSessionRef(value), /Invalid session reference/)
  }
  assert.deepEqual(normalizeSessionRef({ backend: ' codex ', id: ' one ', key: 'spoofed' }), {
    backend: 'codex', id: 'one', key: 'codex:one',
  })
  try {
    installBackendRegistry([null, 42, {}, { id: 'Bad Id', kind: 'codex' },
      { id: 'ept-codex', kind: 'codex' }, { id: 'ept-codex', kind: 'opencode' }])
    assert.deepEqual(BACKEND_IDS, ['codex', 'ept-codex', 'opencode'])
    assert.equal(backendDescriptor('ept-codex').kind, 'codex')
    assert.equal(backendDescriptor('missing').id, 'codex')
    assert.ok(Object.isFrozen(backendDescriptor('ept-codex')))
  } finally { installBackendRegistry() }
})
