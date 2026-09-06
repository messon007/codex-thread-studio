import test from 'node:test'
import assert from 'node:assert/strict'
import { waitForUtilityResult } from './utility-task.mjs'

const defaults = { ensureCurrent() {}, intervalMs: 100, timeoutMs: 300, timeoutMessage: 'timeout', errorMessage: text => text }
test('utility tasks preserve polling intervals and return a typed terminal result', async () => {
  let now = 0, reads = 0
  const delays = []
  const result = await waitForUtilityResult({ ...defaults, now: () => now, sleep: async delay => { delays.push(delay); now += delay }, read: async () => ++reads === 3 ? { status: 'completed', prompt: 'continue' } : { status: 'running' } })
  assert.equal(result.prompt, 'continue')
  assert.deepEqual(delays, [100, 100])
})
test('utility results are discarded if the session changes during a read', async () => {
  let current = true
  await assert.rejects(waitForUtilityResult({ ...defaults, ensureCurrent() { if (!current) throw Error('changed') }, read: async () => { current = false; return { status: 'completed', prompt: 'stale' } } }), /changed/)
})
test('utility failures and timeouts do not continue polling', async () => {
  await assert.rejects(waitForUtilityResult({ ...defaults, read: async () => ({ status: 'failed', error: 'backend failure' }) }), /backend failure/)
  let now = 0
  await assert.rejects(waitForUtilityResult({ ...defaults, now: () => now, sleep: async delay => { now += delay }, read: async () => ({ status: 'running' }) }), /timeout/)
})
