import assert from 'node:assert/strict'
import test from 'node:test'

import { applyCodexNotification, createCodexViewModel } from './codex-native.mjs'

test('assembles streamed plan delta items', () => {
  const model = createCodexViewModel()
  applyCodexNotification(model, {
    method: 'item/plan/delta',
    params: { turnId: 'turn-1', itemId: 'plan-1', delta: 'next step' },
  })

  const item = model.turns[0].items.find((candidate) => candidate.id === 'plan-1')
  assert.equal(item.type, 'plan')
  assert.equal(item.text, 'next step')
})
