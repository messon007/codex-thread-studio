import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  catalogAttentionState,
  codexAttentionState,
  openCodeAttentionState,
  setThreadAttention,
} from './thread-attention.mjs'

test('marks a completed Codex turn or approval as Attention and clears it on the next turn', () => {
  assert.equal(codexAttentionState('turn/completed'), true)
  assert.equal(codexAttentionState('item/commandExecution/requestApproval'), true)
  assert.equal(codexAttentionState('turn/started'), false)
  assert.equal(codexAttentionState('item/agentMessage/delta'), null)
})

test('maps OpenCode idle and permission events to Attention', () => {
  assert.equal(openCodeAttentionState({ type: 'session.idle' }), true)
  assert.equal(openCodeAttentionState({ type: 'permission.asked' }), true)
  assert.equal(openCodeAttentionState({ type: 'session.status', properties: { status: { type: 'busy' } } }), false)
  assert.equal(openCodeAttentionState({ type: 'message.updated', properties: { info: { role: 'user' } } }), false)
})

test('detects catalog running-to-idle transitions without marking initial idle sessions', () => {
  assert.equal(catalogAttentionState({ status: 'running' }, { status: 'idle' }), true)
  assert.equal(catalogAttentionState({ status: 'idle' }, { status: 'running' }), false)
  assert.equal(catalogAttentionState(null, { status: 'idle' }), null)
})

test('acknowledging a session removes its namespaced Attention key', () => {
  const attention = new Set()
  assert.equal(setThreadAttention(attention, 'codex', 'same-id', true), true)
  assert.equal(setThreadAttention(attention, 'opencode', 'same-id', true), true)
  assert.equal(setThreadAttention(attention, 'codex', 'same-id', false), true)
  assert.deepEqual([...attention], ['opencode:same-id'])
})

test('Studio persists Attention and acknowledges it before switching sessions', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /state\.attentionThreads = new Set\(normalizeAttentionThreads\(saved\.attentionThreads\)\)/)
  assert.match(source, /attentionThreads: \[\.\.\.state\.attentionThreads\]/)
  const selectStart = source.indexOf('async function selectThread(')
  const switchBranch = source.indexOf("if (backend !== state.backend)", selectStart)
  const acknowledge = source.indexOf('markThreadAttention(backend, id, false)', selectStart)
  assert.ok(acknowledge > selectStart && acknowledge < switchBranch)
})
