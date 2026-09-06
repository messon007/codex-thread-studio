import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  codexLifecycleEvent,
  codexLifecycleStreamMessage,
  claimLifecycleNotification,
} from './codex-lifecycle-diagnostics.mjs'

test('duplicate lifecycle events remain isolated by backend and status suppression expires', () => {
  const turns = new Map()
  const statuses = new Map()
  let now = 1000
  const claim = (backend, message) => claimLifecycleNotification(backend, message, turns, statuses, () => now)
  const completed = { method: 'turn/completed', params: { threadId: 'one', turn: { id: 'same', status: 'completed' } } }
  assert.ok(claim('codex', completed))
  assert.equal(claim('codex', completed), null)
  assert.ok(claim('ept-codex', completed))
  const status = { method: 'thread/status/changed', params: { threadId: 'one', status: 'idle' } }
  assert.ok(claim('codex', status))
  now += 249
  assert.equal(claim('codex', status), null)
  now += 1
  assert.ok(claim('codex', status))
  turns.clear() // Existing reconnect reset remains caller-owned.
  assert.ok(claim('codex', completed))
})

test('lifecycle decoding rejects malformed envelopes and preserves unknown status strings', () => {
  for (const value of [null, 1, [], {}, { method: 'unrelated' }]) {
    assert.equal(codexLifecycleEvent(value), null)
    assert.equal(codexLifecycleStreamMessage(value), null)
  }
  assert.equal(codexLifecycleEvent({ method: 'thread/status/changed', params: { status: 'future-status' } }).notificationStatus, 'future-status')
  const turns = new Map()
  for (let i = 0; i < 1030; i += 1) {
    claimLifecycleNotification('codex', { method: 'turn/completed', params: { turnId: String(i) } }, turns, new Map(), () => 0)
  }
  assert.equal(turns.size, 1024)
  assert.equal(turns.has('codex:turn/completed:0'), false)
})

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

test('validates backend-tagged lifecycle stream envelopes', () => {
  assert.deepEqual(codexLifecycleStreamMessage({
    method: 'studio/codexLifecycle/ready',
    params: { backends: ['codex', 'ept-codex', null] },
  }), {
    type: 'ready',
    backends: ['codex', 'ept-codex'],
  })
  const message = {
    method: 'turn/completed',
    params: { threadId: 'thread-a', turn: { id: 'turn-1', status: 'completed' } },
  }
  assert.deepEqual(codexLifecycleStreamMessage({
    method: 'studio/codexLifecycle/event',
    params: { backend: 'ept-codex', message },
  }), { type: 'event', backend: 'ept-codex', message })
  assert.equal(codexLifecycleStreamMessage({
    method: 'studio/codexLifecycle/event',
    params: { backend: 'ept-codex', message: { method: 'item/agentMessage/delta' } },
  }), null)
})

test('the app reports lifecycle routing, lag, and cache decisions without message content', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /reportCodexLifecycleNotification\(backend, message,/u)
  assert.match(source, /reportSessionLifecycle\('codex-event-lag'/u)
  assert.match(source, /reportCodexSelectionCacheDecision\(backend, id,/u)
  assert.doesNotMatch(source, /reportCodexLifecycleNotification[\s\S]{0,1600}(?:item\.text|message\.text|params\.delta)/u)
})

test('the cross-backend lifecycle socket survives active backend cleanup and routes explicit origins', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const init = source.slice(source.indexOf('async function init()'), source.indexOf('\nasync function loadBackendRegistry'))
  const cleanup = source.slice(source.indexOf('function cleanupConnections()'), source.indexOf('\nfunction beginTurnLatencyTrace'))
  const lifecycle = source.slice(source.indexOf('function connectCodexLifecycleStream()'), source.indexOf('\nfunction connectBackend'))
  const notification = source.slice(source.indexOf('function handleCodexLifecycleNotification('), source.indexOf('\nfunction handleAppServerMessage'))

  assert.match(init, /connectCodexLifecycleStream\(\)[\s\S]*await codexLifecycleConnection[\s\S]*connectBackend\(\)/u)
  assert.match(lifecycle, /\/ws\/codex-lifecycle/u)
  assert.doesNotMatch(cleanup, /codexLifecycleSocket|codexLifecycleReconnectTimer/u)
  assert.match(notification, /codexNotificationModel\(message, backend\)/u)
  assert.match(notification, /handleQueuedTurnCompletion\([\s\S]*\{ backend, id: threadId \}/u)
  assert.match(notification, /state\.threadsByBackend\[backend\]/u)
  assert.match(notification, /const fullEventCoverage = state\.backend === backend[\s\S]*markCachedModelUnvalidated\(backend, targetModel\)/u)
  assert.match(notification, /refreshOffscreenCodexHistoryAfterCompletion\(backend,/u)
  assert.match(source, /routeCodexNotification\(message, \{[\s\S]*selectedBackend: state\.backend/u)
})

test('offscreen Codex completion refreshes only the changed tail with a full-history fallback', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(app, /codexHistoryLoader\.loadCodexHistoryForBackground\(backend, id, cached\)/u)
  const source = readFileSync(new URL('./codex-history-loader.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('async function loadCodexHistoryForBackground(')
  const loader = source.slice(start)
  assert.match(loader, /thread\/turns\/list/u)
  assert.match(loader, /collectCodexTurnTail/u)
  assert.match(loader, /if \(tail\.matched\)/u)
  assert.match(loader, /thread\/read[\s\S]*includeTurns: true/u)
})
