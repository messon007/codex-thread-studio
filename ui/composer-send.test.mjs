import test from 'node:test'
import assert from 'node:assert/strict'
import { executeComposerSend } from './composer-send.mjs'

test('Composer accepts once and finishes after acknowledgement', async () => {
  const calls = []
  assert.equal(await executeComposerSend({
    send: async () => { calls.push('send'); return { id: 'turn' } },
    acknowledged: result => calls.push(result.id),
    failed: () => assert.fail('unexpected failure'),
    finished: () => calls.push('finished'),
  }), true)
  assert.deepEqual(calls, ['send', 'turn', 'finished'])
})

test('Composer distinguishes transport rejection from post-acceptance UI failure', async () => {
  for (const accepted of [false, true]) {
    const calls = []
    const failure = new Error('failure')
    assert.equal(await executeComposerSend({
      send: async () => { if (!accepted) throw failure; return 'ack' },
      acknowledged: () => { throw failure },
      failed: (error, wasAccepted) => { assert.equal(error, failure); calls.push(wasAccepted) },
      finished: () => calls.push('finished'),
    }), accepted)
    assert.deepEqual(calls, [accepted, 'finished'])
  }
})

test('Composer finishes even if failure reporting throws, without retrying transport', async () => {
  let sends = 0
  let finished = 0
  await assert.rejects(executeComposerSend({
    send: async () => { sends++; throw new Error('send') },
    acknowledged: () => assert.fail('not accepted'),
    failed: () => { throw new Error('report') },
    finished: () => { finished++ },
  }), /report/)
  assert.equal(sends, 1)
  assert.equal(finished, 1)
})
