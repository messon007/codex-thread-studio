import test from 'node:test'
import assert from 'node:assert/strict'

import {
  MAX_MESSAGE_QUEUE_DEPTH,
  completedQueueShouldAdvance,
  normalizeQueueDepth,
  normalizeStoredMessageQueues,
} from './message-queue.mjs'

test('queue depth defaults to one and has a hard maximum of three', () => {
  assert.equal(normalizeQueueDepth(undefined), 1)
  assert.equal(normalizeQueueDepth(2), 2)
  assert.equal(normalizeQueueDepth(MAX_MESSAGE_QUEUE_DEPTH + 1), 1)
})

test('stored queues are bounded and preserve structured input', () => {
  const messages = Array.from({ length: 5 }, (_, index) => ({
    id: `message-${index}`,
    text: `later ${index}`,
    input: [{ type: 'file', path: `later-${index}.md` }],
    createdAt: index + 1,
  }))
  const normalized = normalizeStoredMessageQueues({ 'codex:thread': messages }, () => 99)
  assert.equal(normalized['codex:thread'].length, 3)
  assert.equal(Object.hasOwn(normalized['codex:thread'][0], 'turnOptions'), false)
})

test('only a normal unpaused completion advances a queue', () => {
  assert.equal(completedQueueShouldAdvance('completed', false), true)
  assert.equal(completedQueueShouldAdvance('failed', false), false)
  assert.equal(completedQueueShouldAdvance('completed', true), false)
})
