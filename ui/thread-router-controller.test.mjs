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
  const activity = []
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
      ...overrides.backend,
    },
    catalog: {
      sidebarCatalogs: () => state.threadsByBackend,
      threadTitle: (thread) => thread.name,
      threadStatus: () => 'idle',
      mergeThread: () => {},
      updateLoadedThreadTimestamp: (backend, id, options) => activity.push({ backend, id, options }),
    },
    model: {
      ensureSessionModel: async () => ({ turns: [], status: 'idle' }),
      applyNotification: () => {},
      cacheThreadModel: () => {},
      ...overrides.model,
    },
    view: {
      closeActionMenus: () => {},
      renderThreadList: () => {},
      renderWorkspace: () => {},
      renderTranscript: () => {},
      renderComposerState: () => {},
      renderItem: () => '',
      selectThread: async () => {},
      ...overrides.view,
    },
    persistPreferences: async () => {},
    gatewayFetch: overrides.gatewayFetch,
    notify: overrides.notify,
  })
  return { activity, calls, controller, state }
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
  const { activity, calls, controller, state } = controllerFixture()
  await controller.startTurn('Please prepare a design')
  assert.deepEqual(calls.map((call) => call.type), ['refresh', 'start'])
  const start = calls[1]
  assert.equal(start.ref.id, 'router')
  assert.deepEqual(start.input, [{ type: 'text', text: 'Please prepare a design' }])
  assert.deepEqual(start.options.outputSchema.properties.targetSessionKey.enum, ['codex:worker', ''])
  assert.equal(start.options.turnOptions.effort, 'high')
  assert.ok(start.options.additionalContext['codex-thread-studio/thread-router'])
  assert.equal(state.routerRuntime.pending.has('codex:route-turn'), true)
  assert.equal(state.routerRuntime.dispatches.get('codex:route-turn').status, 'routing')
  assert.deepEqual(activity, [{ backend: 'codex', id: 'router', options: { status: 'active' } }])
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

test('Router acknowledgement stays bound to its original model across a session switch', async () => {
  let releaseRefresh
  let markRefreshStarted
  let fixtureState
  const refreshStarted = new Promise((resolve) => { markRefreshStarted = resolve })
  const refreshReady = new Promise((resolve) => { releaseRefresh = resolve })
  const routerModel = { activeTurnId: null, turns: [] }
  const otherModel = { activeTurnId: null, turns: [] }
  const appliedModels = []
  const cachedModels = []
  const fixture = controllerFixture({
    state: { model: routerModel },
    backend: {
      refreshCatalogs: async () => {
        markRefreshStarted()
        await refreshReady
      },
      configuredTurnOptions: () => ({ selectedAtSend: fixtureState.selectedId }),
    },
    model: {
      applyNotification: (model) => appliedModels.push(model),
      cacheThreadModel: (backend, id, model) => cachedModels.push({ backend, id, model }),
    },
  })
  fixtureState = fixture.state
  const started = fixture.controller.startTurn('Route this safely')
  await refreshStarted
  fixture.state.selectedId = 'worker'
  fixture.state.model = otherModel
  releaseRefresh()
  await started

  assert.deepEqual(appliedModels, [routerModel])
  assert.deepEqual(cachedModels, [{ backend: 'codex', id: 'router', model: routerModel }])
  assert.equal(fixture.calls.at(-1).options.turnOptions.selectedAtSend, 'router')
  assert.equal(otherModel.turns.length, 0)
  for (const timer of fixture.state.routerRuntime.monitors.values()) clearTimeout(timer)
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

test('markReminderReadForTurn resolves the Router dispatch key and stays within the active controller', async () => {
  const view = { renderTranscript: () => {}, refreshTurnNavigator: () => calls.push('refresh-turn-navigator'), renderAttention: () => {} }
  const calls = []
  const { state, controller } = controllerFixture({
    dispatch: { supports: () => true, prepareTurn: async () => {}, startTurn: async () => ({ turn: { id: 'route-turn', status: 'inProgress', items: [] } }) },
    view,
  })
  state.routerRuntime.dispatches.set('codex:route-turn', { status: 'completed', unread: true, decision: { targetSessionKey: 'codex:worker' }, targetTurnId: '1', requestedAt: 1 })
  state.routerRuntime.dispatches.set('codex:other', { status: 'completed', unread: true, decision: { targetSessionKey: 'codex:worker' }, targetTurnId: 'route-turn', requestedAt: 2 })
  state.routerRuntime.controllers.set('codex:route-turn', 'codex:router')
  state.routerRuntime.controllers.set('codex:other', 'codex:other')

  await controller.markReminderReadForTurn('route-turn')
  assert.equal(state.routerRuntime.dispatches.get('codex:route-turn')?.unread, false)
  assert.equal(state.routerRuntime.dispatches.get('codex:other')?.unread, true)
  assert.deepEqual(calls, ['refresh-turn-navigator'])
})

test('markReminderRead consumes persistence failures, restores unread state, and refreshes navigation', async () => {
  const calls = []
  const { state, controller } = controllerFixture({
    gatewayFetch: async () => ({ ok: false }),
    notify: (message, level) => calls.push(['notify', message, level]),
    view: { refreshTurnNavigator: () => calls.push(['refresh-turn-navigator']) },
  })
  state.routerRuntime.dispatches.set('codex:route-turn', { status: 'completed', unread: true, decision: { targetSessionKey: 'codex:worker' }, targetTurnId: 'target-turn', requestedAt: 1 })
  state.routerRuntime.controllers.set('codex:route-turn', 'codex:router')

  await controller.markReminderReadForTurn('route-turn')

  assert.equal(state.routerRuntime.dispatches.get('codex:route-turn')?.unread, true)
  assert.deepEqual(calls, [
    ['notify', 'Unable to save Router delivery state', 'error'],
    ['refresh-turn-navigator'],
  ])
})

test('a visible latest response is acknowledged before it can become an offscreen reminder', async () => {
  const originalDocument = globalThis.document
  const originalRequestAnimationFrame = globalThis.requestAnimationFrame
  const originalCss = globalThis.CSS
  const frames = []
  const response = { getBoundingClientRect: () => ({ top: 120, bottom: 200, height: 80 }) }
  const section = { querySelector: selector => selector.includes('.markdown-body') ? response : null }
  const transcript = {
    querySelector: selector => selector.includes('data-turn-id="route-turn"') ? section : null,
    getBoundingClientRect: () => ({ top: 100, bottom: 500 }),
  }
  const attention = { classList: { toggle() {} }, innerHTML: '' }
  globalThis.document = {
    hidden: false,
    hasFocus: () => true,
    getElementById: id => ({ transcript, 'router-attention': attention })[id] || null,
  }
  globalThis.requestAnimationFrame = callback => { frames.push(callback); return frames.length }
  globalThis.CSS = { escape: value => String(value) }
  try {
    const calls = []
    const { state, controller } = controllerFixture({
      state: { model: { activeTurnId: null, turns: [{ id: 'route-turn' }] } },
      view: { refreshTurnNavigator: () => calls.push('refresh-turn-navigator') },
    })
    state.routerRuntime.dispatches.set('codex:route-turn', { status: 'completed', unread: true, decision: { targetSessionKey: 'codex:worker' }, targetTurnId: 'target-turn', requestedAt: 1 })
    state.routerRuntime.controllers.set('codex:route-turn', 'codex:router')

    controller.renderAttention()
    assert.equal(state.routerRuntime.dispatches.get('codex:route-turn')?.unread, true)
    assert.equal(frames.length, 1)
    frames.shift()()
    await new Promise(resolve => setImmediate(resolve))

    assert.equal(state.routerRuntime.dispatches.get('codex:route-turn')?.unread, false)
    assert.deepEqual(calls, ['refresh-turn-navigator'])

    response.getBoundingClientRect = () => ({ top: 600, bottom: 680, height: 80 })
    controller.renderAttention()
    assert.equal(attention.innerHTML, '')
  } finally {
    globalThis.document = originalDocument
    globalThis.requestAnimationFrame = originalRequestAnimationFrame
    globalThis.CSS = originalCss
  }
})
