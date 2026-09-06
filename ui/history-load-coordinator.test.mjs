import test from 'node:test'
import assert from 'node:assert/strict'
import { coordinateHistoryLoad } from './history-load-coordinator.mjs'

test('resume and refresh share a flight without deferring its start', async () => {
  const loads = new Map()
  let release
  let starts = 0
  const start = () => { starts++; return new Promise(resolve => { release = resolve }) }
  const first = coordinateHistoryLoad(loads, 'codex:one', 'codex', null, start)
  assert.equal(starts, 1)
  assert.equal(coordinateHistoryLoad(loads, 'codex:one', 'codex', null, start), first)
  const other = coordinateHistoryLoad(loads, 'ept-codex:one', 'ept-codex', null, async () => 'other')
  release('done')
  assert.equal(await first, 'done')
  assert.equal(await other, 'other')
  assert.equal(loads.size, 0)
})

test('old OpenCode flight completion cannot remove a newer epoch flight', async () => {
  const loads = new Map()
  let oldRelease, newRelease
  const old = coordinateHistoryLoad(loads, 'opencode:one', 'opencode', 1, () => new Promise(resolve => { oldRelease = resolve }))
  const current = coordinateHistoryLoad(loads, 'opencode:one', 'opencode', 2, () => new Promise(resolve => { newRelease = resolve }))
  oldRelease()
  await old
  assert.equal(loads.get('opencode:one'), current)
  assert.equal(current.historyEpoch, 2)
  newRelease()
  await current
  assert.equal(loads.size, 0)
})

test('failed loads release their slot and retain the original error', async () => {
  const loads = new Map()
  const error = new Error('active writer')
  await assert.rejects(coordinateHistoryLoad(loads, 'codex:one', 'codex', null, async () => { throw error }), value => value === error)
  assert.equal(loads.size, 0)
  assert.equal(await coordinateHistoryLoad(loads, 'codex:one', 'codex', null, async () => 'retry'), 'retry')
})
