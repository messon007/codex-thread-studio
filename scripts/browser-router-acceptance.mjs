// Runs embedded Router code with real DOM and fake transports, without model calls.
export async function checkRouterAcceptance(page, targetBackend = 'codex') {
  return page.evaluate(async targetBackend => {
    const { createThreadRouterController, createThreadRouterRuntimeState } = await import('/thread-router-controller.mjs')
    const { normalizeThreadRouter } = await import('/thread-router.mjs')
    const { presentRoutedTurn } = await import('/transcript-presentation.mjs')
    const check = (condition, message) => { if (!condition) throw Error(message) }
    const headers = { Authorization: `Bearer ${window.__CODEX_THREAD_STUDIO_GATEWAY__.token}`, 'Content-Type': 'application/json' }
    const endpoint = '/studio/router-history?controller=codex%3Asmoke-router'
    check((await fetch(endpoint)).status === 401, 'Router history is not authenticated')
    check((await fetch('/studio/router-history', { method: 'PUT', headers, body: JSON.stringify({ controller: 'codex:smoke-router', turnKey: 'codex:smoke-turn', dispatch: { status: 'completed', targetTurnId: 'smoke-target' } }) })).status === 204, 'Router history write route failed')
    const records = await (await fetch(endpoint, { headers })).json()
    check(records.length === 1 && records[0].dispatch.targetTurnId === 'smoke-target', 'Router history read route failed')
    const threads = [{ id: 'router', name: 'Router', cwd: '/router' }, { id: 'worker', name: 'Worker', cwd: '/worker' }]
    const targetKey = `${targetBackend}:worker`
    const state = {
      backend: 'codex', selectedId: 'router', model: { turns: [], activeTurnId: null },
      threadsByBackend: { codex: threads }, openingMessages: {}, hiddenSessionDirectories: [], sessionDirectoryIgnore: [],
      router: normalizeThreadRouter({ controllerBackend: 'codex', controllers: { codex: 'router' } }),
      routerRuntime: createThreadRouterRuntimeState(), threadModels: new Map(),
    }
    if (targetBackend !== 'codex') state.threadsByBackend[targetBackend] = [threads[1]]
    const workerTurn = { id: 'worker-turn', status: 'completed', items: [{ id: 'reply', type: 'agentMessage', text: 'Worker response' }] }
    const workerModel = { turns: [workerTurn], status: 'idle', activeTurnId: null }
    const saved = new Map(), sent = [], opened = []
    let picked = null
    const noop = () => {}
    const controller = createThreadRouterController({
      state,
      dispatch: {
        supports: () => true, prepareTurn: async () => {},
        startTurn: async (ref, input, options) => { sent.push({ ref, input, options }); return { turn: { id: ref.id === 'router' ? 'route' : 'worker-turn', status: 'inProgress', items: [] } } },
      },
      backend: { refreshCatalogs: async () => { throw Error('Explicit target must not refresh all catalogs') }, configuredTurnOptions: () => ({ model: 'router-model' }), targetTurnOptions: ref => ({ model: `${ref.backend}-worker-model`, effort: 'high' }) },
      catalog: { sidebarCatalogs: () => state.threadsByBackend, threadTitle: thread => thread.name, threadStatus: () => 'idle', updateLoadedThreadTimestamp: noop },
      model: { ensureSessionModel: async () => workerModel, applyNotification: noop, cacheThreadModel: noop },
      view: {
        openTargetPicker: options => { picked = options },
        renderComposerState: noop, renderTranscript: noop, renderWorkspace: noop, renderThreadList: noop,
        renderItem: (item, turnId, options) => `<article data-item-id="${item.id}" data-source="${options?.sourceRef?.key || ''}" data-turn-id="${turnId}">${item.text || ''}</article>`,
        renderTargetTurn: (turn, ref) => presentRoutedTurn(turn).blocks.map(block => block.type === 'assistant' ? `<article data-item-id="${block.item.id}" data-source="${ref.key}" data-turn-id="${turn.id}">${block.item.text}<div class="message-actions"><button data-native-action>Native</button>${controller.renderSourceActions(ref)}</div></article>` : '<details class="work-activity"><summary>Worked</summary></details>').join(''),
        openWorkspaceTool: async (source, tool) => opened.push({ source, tool }),
      },
      gatewayFetch: async (_url, options) => {
        if (options?.method === 'PUT') { const record = JSON.parse(options.body); saved.set(record.turnKey, record); return { ok: true } }
        return { ok: true, json: async () => [...saved.values()] }
      },
      persistPreferences: async () => {}, notify: message => { throw Error(message) },
    })
    const host = document.createElement('section')
    host.innerHTML = '<button id="router-choose-target">Choose</button><textarea id="composer-input">Keep this question</textarea>'
    document.body.append(host)
    try {
      controller.bind()
      host.querySelector('#router-choose-target').click()
      check(picked?.length && host.querySelector('#composer-input').value === 'Keep this question', 'Opening target picker changed the draft or inserted @')
      controller.chooseTarget(targetKey)
      await controller.startTurn('Exact user request')
      const route = state.model.turns[0]
      const routeKey = `codex:${route.id}`
      check(sent.length === 1 && sent[0].ref.id === 'worker', 'explicit target must bypass Router model')
      check(sent[0].ref.backend === targetBackend, 'target backend changed during routing')
      check(sent[0].input[0].text === 'Exact user request', 'explicit prompt was rewritten')
      check(sent[0].options.turnOptions.model === `${targetBackend}-worker-model` && sent[0].options.turnOptions.effort === 'high', 'target model and reasoning configuration were not forwarded')
      await controller.completeTurn({ backend: targetBackend, turnId: 'worker-turn', model: workerModel })
      check(saved.get(routeKey).dispatch.status === 'completed', 'completion was not persisted')
      check(saved.get(routeKey).dispatch.unread === true, 'background completion did not persist unread state')
      controller.renderAttention()
      check(controller.unreadTurnIds().has(route.id), 'background completion missing navigation marker')
      host.innerHTML = controller.renderTurn(route, 0)
      const sourceActions = [...host.querySelectorAll('.message-actions .router-source-action')]
      check(host.querySelector('[data-native-action]').nextElementSibling === sourceActions[0], 'Router actions must follow native actions')
      check(sourceActions.length === 4 && sourceActions.every(button => button.querySelector('svg') && button.title && button.getAttribute('aria-label') === button.title && !button.textContent.trim()), 'Router source actions must be accessible icon-only buttons')
      check(host.textContent.includes('Worker response'), 'worker response not rendered inline')
      check(host.querySelector('[data-item-id="reply"]').dataset.source === targetKey, 'response lost source identity')
      const files = host.querySelector('[data-router-tool="files"]')
      await controller.handleAction(files)
      check(opened[0].source.thread.cwd === '/worker', 'Files opened the wrong directory')
      check(state.selectedId === 'router' && state.routerRuntime.selectedTarget === targetKey, 'tool navigation changed chat target')
      await controller.completeTurn({ backend: 'codex', turnId: route.id, model: { turns: [route] } })
      check(sent.length === 1, 'duplicate notification sent request twice')
      const continued = { id:'supervisor-next', status:'inProgress', items:[{ id:'continued-reply', type:'agentMessage', text:'Continued response' }] }
      workerModel.turns.push(continued)
      await controller.followSupervisedTurn({ backend:targetBackend, id:'worker' }, 'worker-turn', continued.id, workerModel)
      check(saved.get(routeKey).dispatch.targetTurnId === continued.id, 'Router did not follow the supervised task continuation')
      await controller.completeTurn({ backend:targetBackend, turnId:'worker-turn', model:workerModel })
      check(state.routerRuntime.dispatches.get(routeKey).status === 'running', 'Old completion closed the supervised continuation')
      continued.status = 'completed'
      await controller.completeTurn({ backend:targetBackend, turnId:continued.id, model:workerModel })
      check(saved.get(routeKey).dispatch.status === 'completed', 'Supervised continuation completion did not reach Router')
      saved.set('codex:uncertain', { turnKey: 'codex:uncertain', controller: 'codex:router', dispatch: { status: 'dispatching' } })
      state.routerRuntime.dispatches.clear()
      state.routerRuntime.historyLoads.clear()
      state.routerRuntime.directTurns.clear()
      state.model.turns = []
      controller.renderTurn(route, 0)
      await state.routerRuntime.historyLoads.get('codex:router')
      controller.syncDirectTurns()
      check(state.model.turns[0]?.id === route.id, 'refresh lost direct request history')
      host.innerHTML = controller.renderTurn(route, 0)
      check(host.textContent.includes('Continued response'), 'refresh lost the supervised continuation response')
      check(state.routerRuntime.dispatches.get('codex:uncertain').status === 'failed', 'uncertain delivery was not marked for review')
      check(sent.length === 1, 'history restoration replayed a request')
      check(saved.get(routeKey).dispatch.unread === true, 'history restoration incorrectly marked response read')
      return { inlineReply: true, sourceContext: true, exactTarget: true, durableCompletion: true, duplicateProtection: true }
    } finally {
      host.remove()
      for (const timer of state.routerRuntime.monitors.values()) clearTimeout(timer)
    }
  }, targetBackend)
}
