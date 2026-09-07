import test from 'node:test'
import assert from 'node:assert/strict'
import { createTurnSupervision, supervisionResult } from './turn-supervision.mjs'

const ref = { backend: 'codex', id: 'thread' }
function setup(overrides = {}) {
  const snapshot = { turns: [{ id: 'root', status: 'completed', items: [
    { type: 'userMessage', content: [{ type: 'text', text: 'Finish A and B' }] },
    { type: 'agentMessage', text: 'A is done. Shall I do B?' },
  ] }], activeTurnId: '', blocked: false }
  const sends = [], evaluations = []
  const controller = createTurnSupervision({
    snapshot: () => snapshot,
    evaluate: async (_job, input) => { evaluations.push(JSON.parse(input)); return { decision: 'continue', reason: 'B remains', prompt: 'Complete B' } },
    prepare: async () => {},
    send: async (_ref, prompt) => { sends.push(prompt); snapshot.turns.push({ id: 'next', status: 'inProgress', items: [] }); snapshot.activeTurnId = 'next'; return 'next' },
    changed: () => {}, ...overrides,
  })
  return { controller, snapshot, sends, evaluations }
}
test('supervisor only enables on latest eligible turn', () => {
  const { controller, snapshot } = setup()
  assert.throws(() => controller.start(ref, 'old'), /latest/)
  snapshot.blocked = true
  assert.throws(() => controller.start(ref, 'root'), /cannot/)
})

test('an acknowledged Router prompt can seed supervision before userMessage arrives', () => {
  const { controller, snapshot } = setup()
  snapshot.turns[0].items = []
  snapshot.turns[0].status = 'inProgress'
  assert.throws(() => controller.start(ref, 'root'), /original user message/)
  controller.start(ref, 'root', 'Exact accepted request')
  assert.equal(controller.get(ref).original, 'Exact accepted request')
})
test('completion is evaluated once and continuation retains logical root', async () => {
  const { controller, sends, evaluations } = setup()
  controller.start(ref, 'root')
  await Promise.all([controller.tick(), controller.tick(), controller.tick()])
  assert.equal(sends.length, 1)
  assert.equal(evaluations[0].originalUserMessage, 'Finish A and B')
  assert.equal(controller.get(ref).root, 'root')
  assert.equal(controller.get(ref).current, 'next')
  await controller.tick(); assert.equal(sends.length, 1)
})
test('running turns are not evaluated; idle alone is insufficient', async () => {
  const { controller, snapshot, evaluations } = setup()
  snapshot.turns[0].status = 'inProgress'
  controller.start(ref, 'root'); await controller.tick()
  assert.equal(evaluations.length, 0)
})
for (const action of ['turn/interrupt', 'turn/start', 'thread/archive']) {
  test(`${action} during evaluation prevents automatic send`, async () => {
    let release
    const { controller, sends } = setup({ evaluate: () => new Promise(resolve => { release = resolve }) })
    controller.start(ref, 'root'); const tick = controller.tick()
    controller.humanAction(ref, action, {})
    release({ decision: 'continue', prompt: 'Do B', reason: '' }); await tick
    assert.equal(sends.length, 0); assert.equal(controller.activeFor(ref), false)
  })
}
test('Steer invalidates stale evaluation and retains added user context', async () => {
  let release
  const { controller, sends } = setup({ evaluate: () => new Promise(resolve => { release = resolve }) })
  controller.start(ref, 'root'); const tick = controller.tick()
  controller.humanAction(ref, 'turn/steer', { input: [{ text: 'Also C' }] })
  release({ decision: 'continue', prompt: 'Do B', reason: '' }); await tick
  assert.equal(sends.length, 0)
  assert.match(controller.get(ref).additions[0], /Also C/)
  assert.equal(controller.get(ref).status, 'waiting')
})
for (const decision of ['complete', 'needs_user', 'error']) {
  test(`${decision} ends supervision without a continuation`, async () => {
    const { controller, sends } = setup({ evaluate: async () => ({ decision, reason: 'Reason', prompt: '' }) })
    controller.start(ref, 'root'); await controller.tick()
    assert.equal(controller.activeFor(ref), false); assert.equal(sends.length, 0)
  })
}
test('uncertain send is never retried', async () => {
  let attempts = 0
  const { controller } = setup({ send: async () => { attempts++; throw new Error('Timeout') } })
  controller.start(ref, 'root'); await controller.tick(); await controller.tick()
  assert.equal(attempts, 1); assert.equal(controller.get(ref).status, 'stopped')
})
test('approval arriving during preparation prevents dispatch', async () => {
  const harness = setup({ prepare: async () => { harness.snapshot.blocked = true } })
  harness.controller.start(ref, 'root'); await harness.controller.tick()
  assert.equal(harness.sends.length, 0)
})
test('malformed evaluator output fails closed', () => {
  for (const text of ['not json', '{"decision":"continue","reason":"ok","prompt":""}', '{}']) {
    assert.equal(supervisionResult({ turns: [{ status: 'completed', items: [{ type: 'agentMessage', text }] }] }).status, 'failed')
  }
})
test('reload does not inherit or replay supervision', () => {
  const first = setup(); first.controller.start(ref, 'root')
  assert.equal(setup().controller.activeFor(ref), false)
})
test('backend identity isolates otherwise identical thread IDs', () => {
  const { controller } = setup()
  controller.start(ref, 'root')
  const ept = { ...ref, backend: 'ept-codex' }
  controller.start(ept, 'root'); controller.stop(ref)
  assert.equal(controller.activeFor(ept), true)
})
test('errors after enabling stop without evaluation', async () => {
  const { controller, snapshot, evaluations } = setup()
  controller.start(ref, 'root'); snapshot.turns[0].status = 'failed'
  await controller.tick()
  assert.equal(evaluations.length, 0); assert.equal(controller.activeFor(ref), false)
})
test('five continuations are a hard send limit, but completion can still be assessed', async () => {
  let sends = 0
  const harness = setup({
    evaluate: async () => ({ decision: 'continue', reason: '', prompt: `Next step ${sends}` }),
    send: async () => {
      const id = `turn-${++sends}`
      harness.snapshot.turns.push({ id, status: 'completed', items: [{ type: 'agentMessage', text: `Done step ${sends}` }] })
      return id
    },
  })
  harness.controller.start(ref, 'root')
  for (let index = 0; index < 7; index++) await harness.controller.tick()
  assert.equal(sends, 5); assert.equal(harness.controller.get(ref).status, 'stopped')
})
