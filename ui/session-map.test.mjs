import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

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
  structuredWorkerText,
} from './session-map.mjs'

const map = normalizeSessionMap({
  id: 'map-1', backend: 'codex', threadId: 'thread/1', goal: 'Learn', structure: 'hierarchy', revision: 2,
  currentItemId: 'child', items: [
    { id: 'root', title: 'Root', state: 'visited', position: 0 },
    { id: 'child', parentId: 'root', title: 'Child', state: 'active', position: 0 },
    { id: 'archived', title: 'Old', archived: true, position: 1 },
  ],
})

test('Map node menu only exposes current and completed states, without manual editors', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  assert.deepEqual([...html.matchAll(/data-map-item-action="([^"]+)"/gu)].map(match => match[1]), ['current', 'done'])
  assert.doesNotMatch(html, /id="session-map-(?:add-root|edit-goal|item-dialog|goal-dialog)"/u)
  assert.doesNotMatch(html, /id="session-map-create-goal" required/u)
})

test('Map normalization keeps one current item and defaults other nodes to incomplete', () => {
  const input = { id: 'm', backend: 'codex', threadId: 't', currentItemId: 'b', items: [
    { id: 'a', title: 'A', state: 'active' }, { id: 'b', title: 'B', state: 'active' },
    { id: 'c', title: 'C' }, { id: 'd', title: 'D', state: 'done' },
  ] }
  const normalized = normalizeSessionMap(input)
  assert.deepEqual(normalized.items.map(item => item.state), ['notStarted', 'active', 'notStarted', 'done'])
  assert.equal(normalized.currentItemId, 'b')
  input.items[0].state = 'notStarted'
  input.items[1].state = 'done'
  assert.equal(normalizeSessionMap(input).currentItemId, null)
})

test('Map workers recover after rejection and disposed queued jobs cannot run', async () => {
  const pool = new SessionMapWorkerPool()
  const failed = pool.enqueue('map', () => { throw new Error('fixture failure') })
  const next = pool.enqueue('map', () => 42)
  await assert.rejects(failed, /fixture failure/)
  assert.equal(await next, 42)
  const pending = pool.enqueue('map', () => { throw new Error('must not execute') })
  pool.dispose('map')
  await assert.rejects(pending, /released/)
  assert.equal(await pool.enqueue('map', () => 'new worker'), 'new worker')
})

test('Map wire payloads remain unknown until normalized', () => {
  assert.equal(normalizeSessionMap(null), null)
  assert.deepEqual(safeAssistantOperations({ operations: [null, 3, { op: 'setState', itemId: 'x', state: 'done' }] }), [])
  assert.throws(() => parseSessionMapUpdate(`${SESSION_MAP_UPDATE_START}{"baseRevision":"2","operations":[]}${SESSION_MAP_UPDATE_END}`), /missing baseRevision/)
})

test('normalizes and flattens a hierarchy without archived items', () => {
  assert.deepEqual(flattenSessionMap(map).map(({ item, depth }) => [item.id, depth]), [['root', 0], ['child', 1]])
  assert.deepEqual(mapItemTrail(map).map((item) => item.id), ['root', 'child'])
  assert.deepEqual(mapProgress(map), { total: 2, done: 0, explored: 0 })
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
  assert.deepEqual(operations[1], {
    op: 'addItem', itemId: 'next', parentId: null, afterItemId: null,
    title: 'Next', kind: 'item', summary: '', state: 'notStarted',
  })
  const schema = assistantOperationSchema()
  assert.equal(schema.properties.operations.maxItems, 40)
  const operationSchema = schema.properties.operations.items
  assert.equal(operationSchema.oneOf, undefined)
  assert.deepEqual(new Set(operationSchema.required), new Set(Object.keys(operationSchema.properties)))
  const configuration = sessionMapTurnConfiguration(map)
  assert.equal(configuration.dynamicTools[0].name, 'update_session_map')
  assert.match(configuration.developerInstructions, /Current Session Map JSON/)
  assert.match(configuration.developerInstructions, new RegExp(SESSION_MAP_UPDATE_START))
})

test('returns structured worker output and preserves the backend failure reason', () => {
  assert.equal(structuredWorkerText({
    items: [{ type: 'agentMessage', text: '{"operations":[]}' }],
  }), '{"operations":[]}')
  const backendError = JSON.stringify({
    error: { message: "Invalid schema: 'oneOf' is not permitted." },
  })
  assert.throws(
    () => structuredWorkerText({ status: 'failed', items: [], error: { message: backendError } }),
    /oneOf.*not permitted/u,
  )
  assert.throws(() => structuredWorkerText({ status: 'completed', items: [] }), /returned no result/u)
})

test('extracts and hides the inline Map update envelope, including partial streaming markers', () => {
  const visible = 'Normal Markdown response.'
  const payload = { baseRevision: 2, operations: [{ op: 'setCurrent', itemId: 'child' }] }
  const response = `${visible}\n\n${SESSION_MAP_UPDATE_START}\n${JSON.stringify(payload)}\n${SESSION_MAP_UPDATE_END}`
  assert.equal(sessionMapVisibleText(response), visible)
  assert.equal(sessionMapVisibleText(`${visible}\n\n${SESSION_MAP_UPDATE_START.slice(0, 18)}`), visible)
  assert.deepEqual(parseSessionMapUpdate(response), { found: true, visibleText: visible, update: payload })
  assert.deepEqual(parseSessionMapUpdate(visible), { found: false, visibleText: visible, update: null })
  assert.throws(() => parseSessionMapUpdate(`${visible}\n${SESSION_MAP_UPDATE_START}\n{bad json}\n${SESSION_MAP_UPDATE_END}`), /valid JSON/)
  assert.throws(() => parseSessionMapUpdate(`${response}\ntrailing`), /end of the response/)
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
  const update = bootstrapMapInput(map, [{ user: 'Continue studying', assistant: 'Next topic' }])
  assert.match(update, /Update the existing Session Map incrementally/)
  assert.match(update, /Keep existing IDs and completed states/)
  assert.doesNotMatch(update, /4–12 items/)
  assert.match(update, /empty operations array/)
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
