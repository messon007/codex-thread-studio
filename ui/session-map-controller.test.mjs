import test from 'node:test'
import assert from 'node:assert/strict'

import {
  createSessionMapController,
  createSessionMapRuntimeState,
} from './session-map-controller.mjs'

function mapFixture() {
  return {
    id: 'map-one',
    backend: 'codex',
    threadId: 'thread-one',
    goal: 'Understand the project',
    definitionOfDone: '',
    structure: 'hierarchy',
    revision: 1,
    currentItemId: null,
    lastSyncedTurnId: null,
    items: [],
    relations: [],
    createdAt: 1,
    updatedAt: 1,
  }
}

function controllerFixture() {
  const runtime = createSessionMapRuntimeState()
  const state = {
    backend: 'codex',
    selectedId: 'different-thread',
    model: { turns: [] },
    openingMessages: {},
    hiddenCodexThreads: new Set(),
    hiddenCodexTurns: new Set(),
    ready: true,
    artifact: null,
    ...runtime,
  }
  const requests = []
  const sent = []
  const controller = createSessionMapController({
    state,
    transport: {
      gatewayFetch: async (path, options = {}) => {
        requests.push({ path, options })
        const value = options.method === 'POST' && path.endsWith('/operations')
          ? { ...mapFixture(), revision: 2 }
          : mapFixture()
        return { ok: true, status: 200, json: async () => value }
      },
      gatewayWebSocket: () => { throw new Error('not used') },
      rpc: async () => ({}),
      dispatchBackendRpc: async () => ({}),
      sendRaw: (message) => sent.push(message),
    },
    model: {
      createViewModel: () => ({ turns: [] }),
      applyNotification: () => {},
    },
    view: {
      selectedStateKey: () => 'codex:different-thread',
      activateRightWorkspace: () => {},
      closeActionMenus: () => {},
      toggleActionMenu: () => {},
      closeAnnotationRail: () => {},
      closeFavoritesRail: () => {},
      renderArtifact: () => {},
      isResourcesOpen: () => false,
      isWorkspaceOpen: () => false,
    },
    randomId: () => 'generated-id',
  })
  return { controller, requests, sent, state }
}

test('Session Map runtime factories isolate caches, workers, and UI state', () => {
  const first = createSessionMapRuntimeState()
  const second = createSessionMapRuntimeState()
  first.sessionMaps.set('codex:one', mapFixture())
  first.sessionMapDismissed.add('codex:one')
  first.sessionMapSelectedItem = 'item-one'
  assert.equal(second.sessionMaps.size, 0)
  assert.equal(second.sessionMapDismissed.size, 0)
  assert.equal(second.sessionMapSelectedItem, null)
  assert.notEqual(first.sessionMapWorkers, second.sessionMapWorkers)
})

test('Session Map controller coalesces concurrent loads for one session', async () => {
  const { controller, requests, state } = controllerFixture()
  const [first, second] = await Promise.all([
    controller.load('codex', 'thread-one'),
    controller.load('codex', 'thread-one'),
  ])
  assert.equal(requests.length, 1)
  assert.deepEqual(first, second)
  assert.equal(state.sessionMaps.get('codex:thread-one').id, 'map-one')
})

test('Session Map tool updates are validated, persisted, and acknowledged', async () => {
  const { controller, requests, sent, state } = controllerFixture()
  state.sessionMaps.set('codex:thread-one', mapFixture())
  await controller.handleToolCall({
    id: 42,
    params: {
      threadId: 'thread-one',
      turnId: 'turn-one',
      arguments: {
        operations: [{
          op: 'addItem', itemId: 'item-one', parentId: null, afterItemId: null,
          title: 'Inspect architecture', kind: 'topic', summary: '', state: 'notStarted',
        }],
      },
    },
  })
  assert.equal(requests.at(-1).path, '/studio/session-map/codex/thread-one/operations')
  assert.equal(sent[0].result.success, true)
  assert.equal(state.sessionMaps.get('codex:thread-one').revision, 2)
})
