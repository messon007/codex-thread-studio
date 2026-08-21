import test from 'node:test'
import assert from 'node:assert/strict'

import {
  createThreadRouterController,
  createThreadRouterRuntimeState,
} from './thread-router-controller.mjs'
import { normalizeThreadRouter } from './thread-router.mjs'

function controllerFixture(overrides = {}) {
  const threads = [
    { id: 'router', name: 'Thread Router', cwd: '/router' },
    { id: 'worker', name: 'Design worker', cwd: '/project' },
  ]
  const state = {
    backend: 'codex',
    selectedId: 'router',
    model: { activeTurnId: null },
    threads,
    threadsByBackend: { codex: threads },
    openingMessages: { 'codex:worker': { responsibility: 'Design technical specifications' } },
    hiddenSessionDirectories: [],
    sessionDirectoryIgnore: [],
    router: normalizeThreadRouter({ controllerBackend: 'codex', controllers: { codex: 'router' } }),
    routerRuntime: createThreadRouterRuntimeState(),
    ...overrides.state,
  }
  const calls = []
  const dispatch = overrides.dispatch || {
    supports: () => true,
    backends: () => ['codex'],
    prepareTurn: async () => {},
    read: async () => ({ thread: threads[1] }),
    startTurn: async (ref, input, options) => {
      calls.push({ type: 'start', ref, input, options })
      return { turn: { id: 'route-turn', status: 'inProgress', items: [] } }
    },
  }
  const controller = createThreadRouterController({
    state,
    dispatch,
    backend: {
      dispatchRpc: async () => ({}),
      refreshCatalogs: async () => calls.push({ type: 'refresh' }),
      loadBackendInfo: async () => ({ routerWorkspace: '/router' }),
      configuredTurnOptions: () => ({ effort: 'high' }),
    },
    catalog: {
      sidebarCatalogs: () => state.threadsByBackend,
      threadTitle: (thread) => thread.name,
      threadStatus: () => 'idle',
      mergeThread: () => {},
      updateLoadedThreadTimestamp: () => {},
    },
    model: {
      ensureSessionModel: async () => ({ turns: [], status: 'idle' }),
      applyNotification: () => {},
      cacheThreadModel: () => {},
    },
    view: {
      closeActionMenus: () => {},
      renderThreadList: () => {},
      renderWorkspace: () => {},
      renderTranscript: () => {},
      renderComposerState: () => {},
      renderItem: () => '',
      selectThread: async () => {},
    },
    persistPreferences: async () => {},
  })
  return { calls, controller, state }
}

test('Router runtime factories do not share mutable coordination state', () => {
  const first = createThreadRouterRuntimeState()
  const second = createThreadRouterRuntimeState()
  first.pending.set('codex:turn', { requestedAt: 1 })
  first.editor = { fallbacks: [] }
  assert.equal(second.pending.size, 0)
  assert.equal(second.editor, null)
})

test('Router controller refreshes candidates before starting a structured routing turn', async () => {
  const { calls, controller, state } = controllerFixture()
  await controller.startTurn('Please prepare a design')
  assert.deepEqual(calls.map((call) => call.type), ['refresh', 'start'])
  const start = calls[1]
  assert.equal(start.ref.id, 'router')
  assert.deepEqual(start.input, [{ type: 'text', text: 'Please prepare a design' }])
  assert.deepEqual(start.options.outputSchema.properties.targetSessionKey.enum, ['codex:worker'])
  assert.equal(start.options.turnOptions.effort, 'high')
  assert.ok(start.options.additionalContext['codex-thread-studio/thread-router'])
  assert.equal(state.routerRuntime.pending.has('codex:route-turn'), true)
  assert.equal(state.routerRuntime.dispatches.get('codex:route-turn').status, 'routing')
  for (const timer of state.routerRuntime.monitors.values()) clearTimeout(timer)
})

test('Router keeps image attachments with the routing request', async () => {
  const { calls, controller, state } = controllerFixture()
  const image = { type: 'image', url: 'data:image/png;base64,AAAA' }
  await controller.startTurn('Inspect this screenshot', [image])
  assert.deepEqual(calls[1].input, [{ type: 'text', text: 'Inspect this screenshot' }, image])
  assert.deepEqual(state.routerRuntime.pending.get('codex:route-turn').attachments, [image])
  for (const timer of state.routerRuntime.monitors.values()) clearTimeout(timer)
})

test('removing a session repairs Router controllers and fallback references', () => {
  const { controller, state } = controllerFixture({
    state: {
      router: normalizeThreadRouter({
        controllerBackend: 'codex',
        controllers: { codex: 'router' },
        fallbacks: [{ sessionKey: 'codex:worker', condition: 'Use for design work' }],
      }),
    },
  })
  assert.equal(controller.removeSession('codex', 'worker'), true)
  assert.deepEqual(state.router.fallbacks, [])
  assert.equal(controller.removeSession('codex', 'router'), true)
  assert.deepEqual(state.router.controllers, {})
  assert.equal(controller.removeSession('codex', 'missing'), false)
})
