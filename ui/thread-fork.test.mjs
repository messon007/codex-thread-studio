import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  isTurnForkable,
  openCodeForkBody,
  threadForkParams,
} from './thread-fork.mjs'

test('creates backend-neutral fork parameters at a completed turn', () => {
  assert.deepEqual(threadForkParams('thread-1', 'turn-2'), {
    threadId: 'thread-1',
    lastTurnId: 'turn-2',
  })
  assert.deepEqual(threadForkParams('thread-1'), { threadId: 'thread-1' })
})

test('maps the selected OpenCode turn to its user message id', () => {
  assert.deepEqual(openCodeForkBody({ lastTurnId: 'msg-user-2' }), { messageID: 'msg-user-2' })
  assert.deepEqual(openCodeForkBody({}), {})
})

test('offers per-turn forks only after the turn has ended', () => {
  assert.equal(isTurnForkable({ id: 'turn-1', status: 'completed' }), true)
  assert.equal(isTurnForkable({ id: 'turn-1', status: 'failed' }), true)
  assert.equal(isTurnForkable({ id: 'turn-1', status: 'inProgress' }), false)
  assert.equal(isTurnForkable({ id: 'pending', status: 'completed', studioOptimistic: true }), false)
})

test('renders a compact fork action beside each completed response', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.match(source, /data-fork-turn=/u)
  assert.match(source, /data-favorite-message=[\s\S]{0,900}\$\{forkAction\}/u)
  assert.match(readFileSync(new URL('../ui-src/session-operations.mts', import.meta.url), 'utf8'), /rpc\('thread\/fork', threadForkParams\(sourceThreadId, lastTurnId\)\)/u)
  assert.match(styles, /\.message-fork-button/u)
})
