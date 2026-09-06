import assert from 'node:assert/strict'
import test from 'node:test'
import { RouterTurnCoordinator } from './router-coordination.mjs'

function fixture() {
  const state = { pending: new Map(), dispatches: new Map(), targetTurns: new Map() }
  state.pending.set('codex:r', { candidateKeys: ['ept-codex:target'], requestedAt: 1, attachments: [] })
  const turn = { items: [{ type: 'agentMessage', text: JSON.stringify({ action: 'dispatch', targetSessionKey: 'ept-codex:target', forwardedPrompt: 'hello' }) }] }
  return { state, turn, flow: new RouterTurnCoordinator(state) }
}
test('Router duplicate completions dispatch only once, then track the target across backends', async () => {
  const { state, turn, flow } = fixture()
  let resolve, sends = 0
  const gate = new Promise(done => { resolve = done })
  const effects = { changed() {}, failed() { assert.fail('unexpected error') }, started() {}, dispatch: () => { sends++; return gate } }
  const first = flow.complete('codex:r', turn, effects)
  assert.equal(state.dispatches.get('codex:r').status, 'dispatching')
  assert.equal(await flow.complete('codex:r', turn, effects), false)
  resolve({ targetTurnKey: 'ept-codex:t', targetTurnId: 't', targetSessionKey: 'ept-codex:target' })
  assert.equal(await first, true)
  assert.equal(sends, 1)
  assert.equal(state.dispatches.get('codex:r').status, 'running')
  assert.equal(flow.finishTarget('ept-codex:t', { status: 'completed' }), true)
  assert.equal(flow.finishTarget('ept-codex:t', { status: 'completed' }), false)
  assert.equal(state.dispatches.get('codex:r').status, 'completed')
})
test('invalid decisions and failed dispatches retain distinct failure states', async () => {
  for (const invalid of [true, false]) {
    const { state, turn, flow } = fixture()
    const messages = []
    if (invalid) turn.items[0].text = '{}'
    await flow.complete('codex:r', turn, { changed() {}, started() {}, failed: message => messages.push(message), dispatch: async () => { throw Error('target unavailable') } })
    assert.equal(state.dispatches.get('codex:r').status, 'failed')
    assert.equal(state.dispatches.get('codex:r').decisionInvalid, invalid)
    assert.equal(messages.length, 1)
    assert.equal(state.targetTurns.size, 0)
  }
})
test('Router start lock is per session and is released after failures', async () => {
  const { flow } = fixture()
  let reject
  const first = flow.start('codex:r', () => new Promise((_, fail) => { reject = fail }), 'busy')
  await assert.rejects(flow.start('codex:r', async () => 2, 'busy'), /busy/)
  assert.equal(await flow.start('ept-codex:r', async () => 3, 'busy'), 3)
  reject(Error('offline'))
  await assert.rejects(first, /offline/)
  assert.equal(await flow.start('codex:r', async () => 4, 'busy'), 4)
})
