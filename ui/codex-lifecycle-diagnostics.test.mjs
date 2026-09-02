import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import { codexLifecycleEvent } from './codex-lifecycle-diagnostics.mjs'

test('extracts content-free identifiers from Codex lifecycle notifications', () => {
  assert.deepEqual(codexLifecycleEvent({
    method: 'thread/status/changed',
    params: { threadId: 'thread-a', status: { type: 'active', activeFlags: [] } },
  }), {
    method: 'thread/status/changed',
    threadId: 'thread-a',
    turnId: '',
    notificationStatus: 'active',
  })
  assert.deepEqual(codexLifecycleEvent({
    method: 'turn/completed',
    params: { threadId: 'thread-a', turn: { id: 'turn-1', status: 'completed', items: [{ text: 'not logged' }] } },
  }), {
    method: 'turn/completed',
    threadId: 'thread-a',
    turnId: 'turn-1',
    notificationStatus: 'completed',
  })
  assert.equal(codexLifecycleEvent({ method: 'item/completed', params: { threadId: 'thread-a' } }), null)
})

test('the app reports lifecycle routing, lag, and cache decisions without message content', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /reportCodexLifecycleNotification\(backend, message,/u)
  assert.match(source, /reportSessionLifecycle\('codex-event-lag'/u)
  assert.match(source, /reportCodexSelectionCacheDecision\(backend, id,/u)
  assert.doesNotMatch(source, /reportCodexLifecycleNotification[\s\S]{0,1600}(?:item\.text|message\.text|params\.delta)/u)
})
