import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  codexLifecycleEvent,
  codexLifecycleStreamMessage,
} from './codex-lifecycle-diagnostics.mjs'

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
  assert.match(source, /return state\.backend === backend \? state\.model : null/u)
})

test('offscreen Codex completion refreshes only the changed tail with a full-history fallback', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('async function loadCodexHistoryForBackground(')
  const end = source.indexOf('\nasync function resumeThread(', start)
  const loader = source.slice(start, end)
  assert.match(loader, /thread\/turns\/list/u)
  assert.match(loader, /collectCodexTurnTail/u)
  assert.match(loader, /if \(tail\.matched\)/u)
  assert.match(loader, /thread\/read[\s\S]*includeTurns: true/u)
})
