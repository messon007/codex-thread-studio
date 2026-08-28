import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createPerformanceMonitor, exposePerformanceMonitor } from './performance-monitor.mjs'

test('keeps a bounded chronological record of measured work', () => {
  let clock = 10
  const monitor = createPerformanceMonitor({ limit: 2, now: () => clock, wallNow: () => 1_000 })
  const finish = monitor.start('transcript.render', { threadKey: 'codex:a' })
  clock = 13.4567
  finish({ visibleTurns: 30 })
  monitor.record('turnNavigator.render', 4)
  monitor.record('resources.scan', 5)

  const snapshot = monitor.snapshot()
  assert.equal(snapshot.capacity, 2)
  assert.deepEqual(snapshot.records.map((entry) => entry.name), ['turnNavigator.render', 'resources.scan'])
  assert.deepEqual(snapshot.records.map((entry) => entry.sequence), [2, 3])
})

test('summarizes timings and finishes spans only once', () => {
  let clock = 0
  const monitor = createPerformanceMonitor({ now: () => clock, wallNow: () => 10 })
  const finish = monitor.start('history.load', { backend: 'codex' })
  clock = 8
  assert.equal(finish({ cacheHit: false }).durationMs, 8)
  assert.equal(finish(), null)
  monitor.record('history.load', 12, { backend: 'opencode' })

  assert.deepEqual(monitor.snapshot().summary, [{
    name: 'history.load',
    count: 2,
    totalMs: 20,
    averageMs: 10,
    p95Ms: 12,
    maxMs: 12,
  }])
})

test('filters records without retaining unbounded detail values', () => {
  const monitor = createPerformanceMonitor()
  monitor.record('thread.select', 2, {
    threadKey: 'codex:a',
    ignored: { large: true },
    note: 'x'.repeat(500),
  })
  monitor.record('thread.select', 3, { threadKey: 'codex:b' })

  const snapshot = monitor.snapshot({ threadKey: 'codex:a' })
  assert.equal(snapshot.records.length, 1)
  assert.equal(snapshot.records[0].note.length, 240)
  assert.equal('ignored' in snapshot.records[0], false)
})

test('exposes read-only report controls for the developer console', () => {
  const monitor = createPerformanceMonitor()
  const target = {}
  const api = exposePerformanceMonitor(monitor, target)
  monitor.record('transcript.render', 1)

  assert.equal(target.codexThreadStudioPerformance, api)
  assert.equal(api.snapshot().size, 1)
  api.clear()
  assert.equal(api.snapshot().size, 0)
})

test('instruments session switching without retaining transcript content', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  for (const name of [
    'thread.select',
    'history.resume',
    'history.refresh',
    'opencode.history.fetch',
    'transcript.render',
    'transcript.presentation',
    'transcript.dom',
    'transcript.postprocess',
    'turnNavigator.render',
  ]) {
    assert.match(source, new RegExp(`studioPerformance\\.start\\('${name.replaceAll('.', '\\.')}'`, 'u'))
  }
  assert.doesNotMatch(source, /studioPerformance\.start\([^)]*state\.model\.turns/u)
})
