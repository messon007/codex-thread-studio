// Browser-only acceptance harness: real embedded rendering/reducer/controller code,
// isolated DOM and fake transport. Never sends messages to a model or writes settings.
export async function checkComposerAcceptance(page) {
  return page.evaluate(async () => {
    const check = (ok, message) => { if (!ok) throw Error(message) }
    const app = await (await fetch('/app.js')).text()
    const extract = (name, next) => {
      const start = app.indexOf(`function ${name}(`)
      const end = app.indexOf(`\nfunction ${next}(`, start)
      check(start >= 0 && end > start, `Missing renderer ${name}`)
      return app.slice(start, end)
    }
    const { createSubmissionController } = await import('/submission-controller.mjs')
    const { createCodexViewModel, applyCodexNotification } = await import('/codex-native.mjs')
    const { applyOpenCodeEvent } = await import('/opencode-native.mjs')
    const { resolveModelDisplay } = await import('/model-display.mjs')
    const reports = []
    for (const backend of ['codex', 'ept-codex', 'opencode']) {
      const fixture = document.createElement('section')
      fixture.innerHTML = '<textarea id="composer-input"></textarea>' + [
        'composer-model-backend', 'composer-model-name', 'composer-model', 'composer-form',
        'interrupt-turn', 'queue-message', 'continue-thread', 'archive-thread', 'delete-thread',
        'send-message', 'composer-add-image',
      ].map(id => `<button id="${id}"></button>`).join('')
      document.body.append(fixture)
      const $ = selector => fixture.querySelector(selector)
      const state = {
        backend, selectedId: 'fixture', ready: true, model: createCodexViewModel(),
        backendModels: { [backend]: [] }, pendingSkills: {}, pendingFiles: {}, pendingImages: {},
        messageQueues: {}, runningMessageQueues: new Set(), pausedMessageQueues: new Set(), messageQueueErrors: new Map(),
        continuationDraftLoads: new Set(), queueDepth: 1, continueBehavior: 'quickSend',
      }
      const key = `${backend}:fixture`
      const noop = () => {}
      const errors = [], requests = []
      let fail = false
      const dependencies = {
        state, $, currentTurnOptions: () => ({}), currentBackend: () => ({ name: backend, tag: backend }),
        resolveModelDisplay, selectedThread: () => ({}), shellCommandFromComposer: () => null,
        selectedStateKey: () => key, isRouterThread: () => false, isCodexBackend: b => b !== 'opencode',
        composerHasPendingContent: () => Boolean($('#composer-input').value.trim() || state.pendingImages[key]?.length),
        renderComposerImages: noop, renderMessageQueue: noop, reviewNotes: { renderComposerContext: noop },
        runNextQueuedMessage: async () => {}, t: value => value,
      }
      const render = new Function(...Object.keys(dependencies), `${extract('renderComposerState', 'handleContinueAction')}; return renderComposerState`)(...Object.values(dependencies))
      const controller = createSubmissionController(state, {
        ...dependencies, matchingSlashCommands: () => [], isSupportedBackend: () => true,
        backendDescriptor: () => ({ kind: backend === 'opencode' ? 'opencode' : 'codex' }),
        rpc: async (method) => { requests.push(method); return {} },
        dispatchBackendRpc: async (_b, method) => {
          requests.push(method)
          if (fail) throw Error('fixture rejection')
          return { turn: { id: 'turn', status: 'inProgress', items: [] } }
        },
        sessionDispatch: { clearPreparedSession: noop }, prepareComposerTurn: async () => {},
        threadForRef: () => ({ cwd: '/fixture' }), configuredTurnOptions: () => ({}), queuedTurnOptions: () => ({}),
        setComposerDraftValue: (_key, value) => { $('#composer-input').value = value },
        composerDrafts: { value: () => $('#composer-input').value }, hideComposerMenu: noop,
        renderComposerState: render, renderTranscript: noop, beginTranscriptFollowingLatest: noop,
        setCatalogThreadActivity: noop, rollbackCatalogThreadActivity: noop, markCachedModelValidated: noop,
        beginTurnLatencyTrace: noop, bindTurnLatencyTrace: noop, markTurnLatency: noop, finishTurnLatencyTrace: noop,
        randomId: () => 'client', showError: e => errors.push(e), toast: noop,
      })
      const visible = id => !$(id).classList.contains('hidden')
      const idle = () => {
        check(visible('#send-message') && !$('#send-message').disabled, `${backend}: idle Send`)
        check(visible('#continue-thread') && !$('#continue-thread').disabled, `${backend}: idle Continue`)
        check(!visible('#interrupt-turn') && !visible('#queue-message'), `${backend}: idle Stop/Queue`)
      }
      render(); idle()
      $('#composer-input').value = 'hello'
      render()
      check($('#continue-thread').disabled, `${backend}: Continue must not overwrite draft`)
      const sending = controller.sendComposer({ preventDefault: noop })
      if (backend === 'opencode') check($('#send-message').disabled, `${backend}: request in flight disables Send`)
      else check(visible('#interrupt-turn') && $('#send-message').textContent === 'Steer', `${backend}: optimistic running controls`)
      check(await sending, `${backend}: send rejected`)
      check(visible('#interrupt-turn') && visible('#queue-message') && !visible('#continue-thread'), `${backend}: running controls`)
      check(backend === 'opencode' ? !visible('#send-message') : $('#send-message').textContent === 'Steer', `${backend}: Steer visibility`)
      state.messageQueues[key] = [{ id: 'q', text: 'queued', input: [], createdAt: 1 }]
      state.pausedMessageQueues.add(key)
      render()
      check($('#queue-message').textContent === 'Queue (1/1)', `${backend}: queue capacity label`)
      delete state.messageQueues[key]
      state.pausedMessageQueues.delete(key)
      if (backend !== 'opencode') {
        $('#composer-input').value = 'steer'
        await controller.sendComposer({ preventDefault: noop })
        check(requests.includes('turn/steer'), `${backend}: steer dispatch`)
        applyCodexNotification(state.model, { method: 'item/completed', params: { threadId: 'fixture', turnId: 'turn', item: { id: 'answer', type: 'agentMessage', text: 'done' } } })
        render()
        check(visible('#interrupt-turn'), `${backend}: assistant item must not prematurely finish turn`)
        applyCodexNotification(state.model, { method: 'turn/completed', params: { threadId: 'fixture', turn: { id: 'turn', status: 'completed' } } })
      } else {
        applyOpenCodeEvent(state.model, { type: 'session.idle', properties: { sessionID: 'fixture' } }, 'fixture')
      }
      render(); idle()
      state.pendingImages[key] = [{ url: 'fixture' }]
      render()
      check($('#continue-thread').disabled, `${backend}: pending attachment blocks Continue`)
      delete state.pendingImages[key]
      state.ready = false; render()
      check($('#send-message').disabled && $('#continue-thread').disabled, `${backend}: disconnected controls`)
      state.ready = true
      state.model = createCodexViewModel()
      fail = true
      $('#composer-input').value = 'keep me'
      await controller.sendComposer({ preventDefault: noop })
      check(errors.length === 1 && $('#composer-input').value === 'keep me', `${backend}: rejection preserves draft`)
      check(!visible('#interrupt-turn') && !$('#send-message').disabled, `${backend}: rejected send unlocks controls`)
      reports.push(`${backend}: idle, draft, send, running, completion, disconnect, rejection PASS`)
      fixture.remove()
    }
    return reports
  })
}

export async function checkCommentMarkerScope(page) {
  return page.evaluate(async () => {
    const { createReviewNotesController, createReviewNotesState } = await import('/review-notes-controller.mjs')
    const root = document.createElement('section')
    root.innerHTML = '<div id="transcript"><div id="fixture-item"><div class="markdown-body">sample text</div></div></div><div id="artifact-content" class="markdown-body">sample text</div>'
    document.body.append(root)
    const state = { ...createReviewNotesState(), selectedId: 'fixture' }
    const view = { selectedStateKey: () => 'fixture', renderedItem: () => root.querySelector('#fixture-item') }
    const controller = createReviewNotesController({ state, view, commentSources: {}, gatewayFetch() {}, randomId: () => 'x', persistAnnotationState() {} })
    state.annotationDrafts.fixture = [{ id: 'doc', excerpt: 'sample', source: { provider: 'document', anchor: { filePath: '/fixture.md' } } }]
    controller.renderCommentMarkers()
    if (root.querySelector('.chat-comment-anchor')) throw Error('Document comment unexpectedly used a chat marker')
    state.annotationDrafts.fixture.push({ id: 'chat', excerpt: 'sample', source: { provider: 'chat', anchor: { turnId: 't', itemId: 'i' } } })
    controller.renderCommentMarkers()
    if (!root.querySelector('#transcript .chat-comment-anchor')) throw Error('Chat marker missing')
    if (root.querySelector('#artifact-content .chat-comment-anchor')) throw Error('Unexpected document marker')
    root.remove()
    return 'PASS: chat markers render; document preview has no marker implementation (existing limitation)'
  })
}
