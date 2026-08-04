import test from 'node:test'
import assert from 'node:assert/strict'

import {
  assistantOperationSchema,
  bootstrapMapInput,
  boundedMapContext,
  flattenSessionMap,
  mapItemTrail,
  mapProgress,
  normalizeSessionMap,
  parseSessionMapUpdate,
  safeAssistantOperations,
  SESSION_MAP_UPDATE_END,
  SESSION_MAP_UPDATE_START,
  SessionMapWorkerPool,
  sessionMapTurnConfiguration,
  sessionMapEndpoint,
  shouldBootstrapSessionMap,
  sessionMapVisibleText,
} from './session-map.mjs'

const map = normalizeSessionMap({
  id: 'map-1', backend: 'codex', threadId: 'thread/1', goal: 'Learn', structure: 'hierarchy', revision: 2,
  currentItemId: 'child', items: [
    { id: 'root', title: 'Root', state: 'visited', position: 0 },
    { id: 'child', parentId: 'root', title: 'Child', state: 'active', position: 0 },
    { id: 'archived', title: 'Old', archived: true, position: 1 },
  ],
})

test('normalizes and flattens a hierarchy without archived items', () => {
  assert.deepEqual(flattenSessionMap(map).map(({ item, depth }) => [item.id, depth]), [['root', 0], ['child', 1]])
  assert.deepEqual(mapItemTrail(map).map((item) => item.id), ['root', 'child'])
  assert.deepEqual(mapProgress(map), { total: 2, done: 0, explored: 1 })
})

test('builds encoded REST endpoints and bounded model context', () => {
  assert.equal(sessionMapEndpoint('codex', 'thread/1', 'operations'), '/studio/session-map/codex/thread%2F1/operations')
  const context = JSON.parse(boundedMapContext(map))
  assert.equal(context.currentItemId, 'child')
  assert.equal(context.items.length, 2)
  assert.equal(context.items.some((item) => item.id === 'archived'), false)
})

test('filters automatic operations to the safe protocol subset', () => {
  const operations = safeAssistantOperations({ operations: [
    { op: 'setCurrent', itemId: 'child' },
    { op: 'setState', itemId: 'child', state: 'done' },
    { op: 'archiveItem', itemId: 'root' },
    { op: 'addItem', itemId: 'next', title: 'Next', state: 'notStarted' },
  ] })
  assert.deepEqual(operations.map((operation) => operation.op), ['setCurrent', 'addItem'])
  assert.equal(assistantOperationSchema().properties.operations.maxItems, 40)
  const configuration = sessionMapTurnConfiguration(map)
  assert.equal(configuration.dynamicTools[0].name, 'update_session_map')
  assert.match(configuration.developerInstructions, /Current Session Map JSON/)
  assert.match(configuration.developerInstructions, new RegExp(SESSION_MAP_UPDATE_START))
})

test('extracts and hides the inline Map update envelope, including partial streaming markers', () => {
  const visible = 'Normal Markdown response.'
  const payload = { baseRevision: 2, operations: [{ op: 'setCurrent', itemId: 'child' }] }
  const response = `${visible}\n\n${SESSION_MAP_UPDATE_START}\n${JSON.stringify(payload)}\n${SESSION_MAP_UPDATE_END}`
  assert.equal(sessionMapVisibleText(response), visible)
  assert.equal(sessionMapVisibleText(`${visible}\n\n${SESSION_MAP_UPDATE_START.slice(0, 18)}`), visible)
  assert.deepEqual(parseSessionMapUpdate(response), { found: true, visibleText: visible, update: payload })
  assert.deepEqual(parseSessionMapUpdate(visible), { found: false, visibleText: visible, update: null })
  assert.throws(() => parseSessionMapUpdate(`${visible}\n${SESSION_MAP_UPDATE_START}\n{bad json}\n${SESSION_MAP_UPDATE_END}`), /有效 JSON/)
  assert.throws(() => parseSessionMapUpdate(`${response}\ntrailing`), /回复结尾/)
})

test('bootstraps only an empty unsynchronized Map and includes recent conversation context', () => {
  const empty = normalizeSessionMap({
    id: 'empty-map', backend: 'codex', threadId: 'thread-2', goal: 'Understand pending updates',
    definitionOfDone: 'Every package has an upgrade decision', structure: 'hierarchy', items: [],
  })
  assert.equal(shouldBootstrapSessionMap(empty), true)
  assert.equal(shouldBootstrapSessionMap({ ...empty, lastSyncedTurnId: 'turn-1' }), false)
  assert.equal(shouldBootstrapSessionMap(map), false)
  const input = bootstrapMapInput(empty, [{ user: 'Which packages need updates?', assistant: 'There are several packages.' }])
  assert.match(input, /Understand pending updates/)
  assert.match(input, /Which packages need updates\?/)
  assert.match(input, /4–12 items/)
  const oversized = bootstrapMapInput(empty, Array.from({ length: 12 }, () => ({ user: 'u'.repeat(8_000), assistant: 'a'.repeat(12_000) })))
  assert.ok(oversized.length < 55_000)
})

test('reuses one worker per Map, serializes jobs, and resets only across server generations', async () => {
  const pool = new SessionMapWorkerPool()
  const order = []
  let releaseFirst
  const firstGate = new Promise((resolve) => { releaseFirst = resolve })
  const first = pool.enqueue('codex:thread-1', async (worker) => {
    worker.threadId = 'worker-1'
    order.push('first:start')
    await firstGate
    order.push('first:end')
    return worker.threadId
  })
  const second = pool.enqueue('codex:thread-1', async (worker) => {
    order.push('second')
    return worker.threadId
  })
  await Promise.resolve()
  assert.deepEqual(order, ['first:start'])
  releaseFirst()
  assert.deepEqual(await Promise.all([first, second]), ['worker-1', 'worker-1'])
  assert.deepEqual(order, ['first:start', 'first:end', 'second'])

  const worker = pool.workers.get('codex:thread-1')
  worker.generation = 7
  assert.equal(pool.reconcileGeneration(worker, 7), null)
  assert.equal(worker.threadId, 'worker-1')
  assert.equal(pool.reconcileGeneration(worker, 8), 'worker-1')
  assert.equal(worker.threadId, null)
  assert.equal(pool.dispose('codex:thread-1'), worker)
  assert.equal(pool.workers.size, 0)
})
