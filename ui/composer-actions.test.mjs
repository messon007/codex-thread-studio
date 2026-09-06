import assert from 'node:assert/strict'
import test from 'node:test'
import { createComposerActions } from './composer-actions.mjs'
import { createCodexViewModel } from './codex-native.mjs'

function fixture() {
  const state = { backend: 'codex', selectedId: 'a', ready: true, socketGeneration: 1, model: createCodexViewModel(),
    pendingImages: {}, pendingSkills: {}, pendingFiles: {}, messageQueues: {}, pausedMessageQueues: new Set(), runningMessageQueues: new Set(), messageQueueErrors: new Map(),
    continuationDraftLoads: new Set(), continueBehavior: 'ollamaDraft', translation: { ollamaModel: 'gemma3:4b' }, queueDepth: 1 }
  const calls = [], elements = new Map()
  const $ = id => {
    if (!elements.has(id)) elements.set(id, { value: '', disabled: false, classList: { contains: () => false, toggle() {} }, requestSubmit: () => calls.push('submit'), setSelectionRange() {}, focus() {}, showModal() {}, close: () => calls.push('close'), scrollIntoView: () => calls.push('scroll') })
    return elements.get(id)
  }
  const services = { $, selectedStateKey: () => `${state.backend}:${state.selectedId}`, composerDrafts: { value: () => $('#composer-input').value },
    setCurrentComposerValue: value => { $('#composer-input').value = value }, hideComposerMenu() {}, latestAgentResponseText: () => 'last answer',
    showError: error => calls.push(error.message), t: s => s, currentBackend: () => ({ name: 'Codex' }), renderComposerState: () => calls.push('render'), toast: s => calls.push(s),
    setComposerDraftValue: (key, text) => { if (key === `${state.backend}:${state.selectedId}`) $('#composer-input').value = text },
    gatewayFetch: async () => new Response(JSON.stringify({ prompt: 'continue draft' })), truncateCharacters: (s, n) => s.slice(0, n),
    randomId: () => 'q', persistMessageQueue: async () => calls.push('persist'),
    rpc: async () => { calls.push('interrupt'); return {} }, pauseMessageQueue: () => { calls.push('pause'); state.pausedMessageQueues.add('codex:a') },
    sessionRefFromKey: () => ({ backend: state.backend, id: state.selectedId }), runNextQueuedMessage: async () => calls.push('run'),
  }
  return { state, calls, $, services, controller: () => createComposerActions(state, services) }
}
test('queue capacity is enforced and draft clears only after successful persistence', async () => {
  const f = fixture(), c = f.controller()
  f.$('#composer-input').value = 'queued'
  await c.queueComposerMessage()
  assert.equal(f.state.messageQueues['codex:a'][0].text, 'queued')
  assert.equal(f.$('#composer-input').value, '')
  f.$('#composer-input').value = 'second'
  await c.queueComposerMessage()
  assert.equal(f.state.messageQueues['codex:a'].length, 1)
  assert.equal(f.$('#composer-input').value, 'second')
  assert.ok(f.calls.includes('scroll'))
})
test('rejected queue save restores pause/error state and retains attachments and draft', async () => {
  const f = fixture()
  f.services.persistMessageQueue = async () => { throw Error('disk full') }
  f.state.pausedMessageQueues.add('codex:a'); f.state.messageQueueErrors.set('codex:a', 'old')
  f.state.pendingFiles['codex:a'] = [{ type: 'file', path: '/fixture' }]
  f.$('#composer-input').value = 'keep'
  await f.controller().queueComposerMessage()
  assert.equal(f.$('#composer-input').value, 'keep')
  assert.equal(f.state.pendingFiles['codex:a'].length, 1)
  assert.ok(f.state.pausedMessageQueues.has('codex:a'))
  assert.equal(f.state.messageQueueErrors.get('codex:a'), 'old')
})
test('queue edit/delete failures restore the original message', async () => {
  const f = fixture(), original = { id: 'q', text: 'old', input: [{ type: 'file', path: '/a' }], createdAt: 1 }
  f.state.messageQueues['codex:a'] = [original]
  f.services.persistMessageQueue = async () => { throw Error('failed') }
  const c = f.controller(); c.openQueuedMessageEditor('q')
  f.$('#edit-queued-message-text').value = 'edited'
  await c.saveEditedQueuedMessage({ preventDefault() {} })
  assert.equal(original.text, 'old')
  assert.equal(original.input.length, 1)
  await assert.rejects(c.deleteQueuedMessage('q'), /failed/)
  assert.equal(f.state.messageQueues['codex:a'][0], original)
})
test('Continue drafts fill the composer without sending', async () => {
  const f = fixture()
  await f.controller().draftContinueMessage()
  assert.equal(f.$('#composer-input').value, 'continue draft')
  assert.equal(f.calls.includes('submit'), false)
  assert.equal(f.state.continuationDraftLoads.size, 0)
})
test('Continue discards stale results after typing or switching sessions', async () => {
  for (const change of ['typing', 'selection']) {
    const f = fixture(); let resolve
    f.services.gatewayFetch = () => new Promise(r => { resolve = r })
    const pending = f.controller().draftContinueMessage()
    f.$('#composer-input').value = 'new draft'
    if (change === 'selection') f.state.selectedId = 'b'
    resolve(new Response(JSON.stringify({ prompt: 'stale draft' })))
    await pending
    assert.equal(f.$('#composer-input').value, 'new draft')
    assert.equal(f.state.continuationDraftLoads.size, 0)
  }
})
test('quick Continue submits once and Stop pauses its queue before the RPC', async () => {
  const f = fixture(), c = f.controller()
  c.quickSendContinueMessage()
  assert.ok(f.$('#composer-input').value)
  assert.equal(f.calls.filter(v => v === 'submit').length, 1)
  f.state.model.activeTurnId = 'turn'
  await c.interruptTurn()
  assert.ok(f.calls.indexOf('pause') < f.calls.indexOf('interrupt'))
  assert.ok(f.state.pausedMessageQueues.has('codex:a'))
})
