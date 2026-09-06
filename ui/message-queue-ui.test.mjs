import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')

test('active turns expose Queue independently from Stop and Steer', () => {
  assert.match(html, /id="interrupt-turn"[\s\S]*id="queue-message"[\s\S]*id="send-message"/u)
  assert.match(app, /queueAvailable = active && !shellMode && !isRouterThread\(\)/u)
  assert.match(app, /classList\.toggle\('hidden', active && state\.backend === 'opencode'\)/u)
})

test('queue advancement uses terminal backend events rather than polling', () => {
  assert.match(app, /message\.method === 'turn\/completed'[\s\S]*handleQueuedTurnCompletion/u)
  assert.match(app, /payload\.type === 'session\.idle'[\s\S]*handleQueuedTurnCompletion/u)
  assert.doesNotMatch(app, /setInterval\([^)]*runNextQueuedMessage/u)
})

test('persisted queues start paused and use a bounded session-state endpoint', () => {
  assert.match(app, /pausedMessageQueues = new Set\(Object\.keys\(state\.messageQueues\)\)/u)
  const persistence = readFileSync(new URL('./session-state-persistence.mjs', import.meta.url), 'utf8')
  assert.match(persistence, /'\/studio\/session-state\/message-queue'/u)
  assert.match(app, /sessionPersistence\.messageQueue\(key\)/u)
  assert.match(html, /id="queue-depth"[\s\S]*value="1"[\s\S]*value="3"/u)
})
