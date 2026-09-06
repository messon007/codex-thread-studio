import test from 'node:test'
import assert from 'node:assert/strict'
import { normalizeStoredTurnOptions, copySessionTurnOptions, sessionModelPreferencePayload } from './session-model-preferences.mjs'

test('session models restore independently and retain unavailable concrete model IDs', () => {
  const stored = normalizeStoredTurnOptions({
    'codex:one': { model: 'gpt-model', effort: 'high', sandboxPolicy: {} },
    'ept-codex:one': { model: 'removed-model', effort: 'low' },
    'opencode:one': { model: 'provider/model' },
  })
  assert.deepEqual(stored['codex:one'], { model: 'gpt-model', effort: 'high' })
  assert.equal(stored['ept-codex:one'].model, 'removed-model')
  assert.equal(stored['opencode:one'].model, 'provider/model')
  assert.deepEqual(sessionModelPreferencePayload('codex:one', stored['codex:one']), {
    sessionKey: 'codex:one', model: 'gpt-model', effort: 'high',
  })
})

test('queued dispatch reads the latest session model and returns an independent options object', () => {
  const stored = { 'codex:one': { model: 'previous' } }
  stored['codex:one'] = { model: 'current-backend-default', effort: 'medium' }
  const options = copySessionTurnOptions(stored, 'codex:one', { effort: 'high' })
  assert.equal(options.model, 'current-backend-default')
  options.model = 'changed-copy'
  assert.equal(stored['codex:one'].model, 'current-backend-default')
  assert.deepEqual(copySessionTurnOptions(stored, 'codex:missing', { effort: 'high' }), { effort: 'high' })
  assert.deepEqual(sessionModelPreferencePayload('codex:missing'), { sessionKey: 'codex:missing', model: '', effort: '' })
})

test('stored preferences bound and sanitize untrusted JSON without persisting transient permissions', () => {
  for (const input of [null, [], 'invalid', 12]) assert.deepEqual(normalizeStoredTurnOptions(input), {})
  assert.deepEqual(normalizeStoredTurnOptions({ invalid: { model: 'x' }, 'codex:bad': { model: 'a\nb' }, 'codex:empty': {} }), {})
  const result = normalizeStoredTurnOptions({ 'codex:long': { model: 'x'.repeat(300), effort: 'y'.repeat(90) } })
  assert.equal(result['codex:long'].model.length, 256)
  assert.equal(result['codex:long'].effort.length, 64)
})
