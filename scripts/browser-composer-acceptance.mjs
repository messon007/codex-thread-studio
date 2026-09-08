// Browser-only acceptance harness: real embedded rendering/reducer/controller code,
// isolated DOM and fake transport. Never sends messages to a model or writes settings.
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
export async function checkComposerAcceptance(page) {
  const screenshotDirectory = process.env.STUDIO_SCREENSHOT_DIR
  if (screenshotDirectory) {
    await mkdir(screenshotDirectory, { recursive: true })
    await page.exposeFunction('captureComposerAcceptance', async name => {
      if (!/^[a-z0-9-]+$/.test(name)) throw Error('Invalid screenshot name')
      await page.locator('[data-composer-acceptance]').screenshot({ path: join(screenshotDirectory, `${name}.png`), animations: 'disabled' })
    })
  }
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
    const { createComposerActions } = await import('/composer-actions.mjs')
    const { createActiveCodexConnection } = await import('/active-codex-connection.mjs')
    const { createCodexViewModel, applyCodexNotification } = await import('/codex-native.mjs')
    const { applyOpenCodeEvent } = await import('/opencode-native.mjs')
    const { resolveModelDisplay } = await import('/model-display.mjs')
    const reports = []
    for (const backend of ['codex', 'ept-codex', 'opencode']) {
      const fixture = document.createElement('section')
      fixture.dataset.composerAcceptance = backend
      fixture.style.cssText = 'width:780px;padding:24px;background:white;color:#243536;font:14px sans-serif'
      fixture.innerHTML = '<form id="composer-form"><textarea id="composer-input"></textarea>' + [
        'composer-model-backend', 'composer-model-name', 'composer-model',
        'interrupt-turn', 'queue-message', 'continue-thread', 'archive-thread', 'delete-thread',
        'send-message', 'composer-add-image',
      ].map(id => `<button type="button" id="${id}"></button>`).join('')
        + '</form><span id="native-connection"></span><section id="composer-message-queue"></section><dialog id="edit-queued-message-dialog"><textarea id="edit-queued-message-text"></textarea><p id="edit-queued-message-note"></p></dialog>'
      fixture.insertAdjacentHTML('afterbegin', '<h3 data-capture-title></h3><style>[data-composer-acceptance] .hidden{display:none!important}[data-composer-acceptance] textarea{display:block;width:100%;height:92px;margin-bottom:16px;box-sizing:border-box}[data-composer-acceptance] button{margin:4px;padding:8px 12px;border:1px solid #c9d8d2;border-radius:7px;background:#eef5f1;color:#224f42}[data-composer-acceptance] button:disabled{opacity:.4;color:#727b78;background:#eee}</style>')
      document.body.append(fixture)
      const $ = selector => fixture.querySelector(selector)
      const state = {
        backend, selectedId: 'fixture', ready: true, socketGeneration: 1, model: createCodexViewModel(),
        backendModels: { [backend]: [] }, pendingSkills: {}, pendingFiles: {}, pendingImages: {},
        messageQueues: {}, runningMessageQueues: new Set(), pausedMessageQueues: new Set(), messageQueueErrors: new Map(),
        continuationDraftLoads: new Set(), queueDepth: 1, continueBehavior: 'quickSend',
      }
      const key = `${backend}:fixture`
      const capture = async phase => {
        fixture.setAttribute('aria-label', `${backend}: ${phase}`)
        fixture.querySelector('[data-capture-title]').textContent = `${backend} · ${phase}`
        $('#archive-thread').textContent = 'Archive'; $('#delete-thread').textContent = 'Delete'; $('#composer-add-image').textContent = 'Attach'
        if (window.captureComposerAcceptance) await window.captureComposerAcceptance(`${backend}-${phase}`)
      }
      const noop = () => {}
      const errors = [], requests = []
      let fail = false
      const dependencies = {
        state, $, currentTurnOptions: () => ({}), currentBackend: () => ({ name: backend, tag: backend }),
        resolveModelDisplay, selectedThread: () => ({}), shellCommandFromComposer: () => null,
        selectedStateKey: () => key, isRouterThread: () => Boolean(state.routerMode), isCodexBackend: b => b !== 'opencode',
        composerHasPendingContent: () => Boolean($('#composer-input').value.trim() || state.pendingImages[key]?.length || state.pendingFiles[key]?.length || state.pendingSkills[key]?.length),
        renderComposerTools: noop, renderComposerImages: noop, renderMessageQueue: noop, reviewNotes: { renderComposerContext: noop },
        threadRouter: { renderComposerTarget: noop },
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
      state.ready = false; render()
      check($('#send-message').disabled && $('#continue-thread').disabled, `${backend}: connecting controls`)
      await capture('connecting')
      if (backend !== 'opencode') {
        Object.assign(state, { socketGeneration: 1, appServerGenerations: {}, threadModels: new Map(), threadLoads: new Map() })
        const loads = new Map()
        let finishCatalog
        const connection = createActiveCodexConnection(state, {
          $, backendDescriptor: () => ({ name: backend }), codexBackendsNeedingRestartRecovery: new Set(),
          sessionDispatch: { clearPrepared: noop }, clearStartedThreadsForBackend: noop,
          setBackendState: noop, setNativeError: noop, loadBackendModels: async () => {},
          loadThreads: () => new Promise(resolve => { finishCatalog = resolve }), backendSelectionLoads: loads,
          sessionManagement: { archive: { isOpen: () => false } }, threadCatalogKey: (b, id) => `${b}:${id}`,
          freshThreadModel: () => true, handleThreadCatalogFailure: error => { throw error },
          renderComposerState: render,
        })
        connection.handleAppServerMessage({ method: 'studio/appServer/status', params: { state: 'ready', generation: 1 } })
        // No manual render here: the real ready handler must refresh cached controls.
        idle(); finishCatalog(); await Promise.all(loads.values())
        connection.handleAppServerMessage({ method: 'studio/appServer/status', params: { state: 'stopped' } })
        check($('#send-message').disabled && $('#continue-thread').disabled && !state.ready, `${backend}: stopped must revoke sending`)
        const reconnected = connection.handleAppServerMessage({ method: 'studio/appServer/status', params: { state: 'ready', generation: 1 } })
        void reconnected
        idle(); finishCatalog(); await Promise.all(loads.values())
      } else {
        // Exercise the actual OpenCode startup function with cached catalog no-op.
        const body = app.slice(app.indexOf('async function connectOpenCode('), app.indexOf('\nfunction cleanupSocket(', app.indexOf('async function connectOpenCode(')))
        const deps = { state, clearTimeout, cleanupConnections: noop, setBackendState: noop, setNativeError: noop,
          renderComposerState: render, loadBackendInfo: async () => {}, loadBackendModels: async () => {},
          startOpenCodeEventStream: noop, waitForOpenCodeEventStream: async () => true, openCodeStreamState: {},
          renderOpenCodeConnectionState: noop, loadThreads: async () => {}, backendSelectionLoads: new Map(), handleThreadCatalogFailure: error => { throw error } }
        await new Function(...Object.keys(deps), `${body}; return connectOpenCode`)(...Object.values(deps))({ backendInfoReady: true })
        idle()
      }
      await capture('ready-cached')
      state.model.status = 'running'; state.model.activeTurnId = null; render()
      check($('#send-message').disabled && $('#continue-thread').disabled && $('#interrupt-turn').disabled, `${backend}: missing native Turn ID must block sends and Stop`)
      await capture('awaiting-turn-id')
      state.model.status = 'idle'; render(); idle()
      state.pendingFiles[key] = [{ path: '/fixture/example.txt' }]; render()
      check(!$('#send-message').disabled && $('#continue-thread').disabled, `${backend}: file blocks Continue, not Send`)
      await capture('file-attached')
      delete state.pendingFiles[key]; render(); idle()
      state.pendingImages[key] = [{ name: 'example.png' }]; render()
      check($('#continue-thread').disabled, `${backend}: image blocks Continue`)
      delete state.pendingImages[key]; render(); idle()
      await capture('attachments-removed')
      $('#composer-input').value = 'hello'
      render()
      check($('#continue-thread').disabled, `${backend}: Continue must not overwrite draft`)
      await capture('draft')
      $('#composer-input').value = ''; render(); idle()
      $('#composer-input').value = 'hello'; render()
      const sending = controller.sendComposer({ preventDefault: noop })
      if (backend === 'opencode') check($('#send-message').disabled, `${backend}: request in flight disables Send`)
      else check(visible('#interrupt-turn') && $('#send-message').textContent === 'Steer', `${backend}: optimistic running controls`)
      check(await sending, `${backend}: send rejected`)
      check(visible('#interrupt-turn') && visible('#queue-message') && !visible('#continue-thread'), `${backend}: running controls`)
      check(backend === 'opencode' ? !visible('#send-message') : $('#send-message').textContent === 'Steer', `${backend}: Steer visibility`)
      await capture('running')
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
      await capture('completed')
      state.pendingImages[key] = [{ url: 'fixture' }]
      render()
      check($('#continue-thread').disabled, `${backend}: pending attachment blocks Continue`)
      delete state.pendingImages[key]
      state.ready = false; render()
      check($('#send-message').disabled && $('#continue-thread').disabled, `${backend}: disconnected controls`)
      await capture('disconnected')
      state.ready = true
      state.model = createCodexViewModel()
      fail = true
      $('#composer-input').value = 'keep me'
      await controller.sendComposer({ preventDefault: noop })
      check(errors.length === 1 && $('#composer-input').value === 'keep me', `${backend}: rejection preserves draft`)
      check(!visible('#interrupt-turn') && !$('#send-message').disabled, `${backend}: rejected send unlocks controls`)
      let submissions = 0
      $('#composer-form').addEventListener('submit', event => { event.preventDefault(); submissions++ })
      const actionRequests = []
      const actions = createComposerActions(state, {
        ...dependencies, composerDrafts: { value: () => $('#composer-input').value },
        setCurrentComposerValue: text => { $('#composer-input').value = text },
        setComposerDraftValue: (_key, text) => { $('#composer-input').value = text },
        hideComposerMenu: noop, renderComposerState: render, toast: noop, showError: error => { throw error },
        randomId: () => 'queued', persistMessageQueue: async () => {},
        pauseMessageQueue: () => state.pausedMessageQueues.add(key),
        rpc: async (method) => { actionRequests.push(method); return {} },
        latestAgentResponseText: () => 'last reply', truncateCharacters: (s, n) => s.slice(0, n),
        gatewayFetch: async () => new Response(JSON.stringify({ prompt: 'reviewed continuation' })),
      })
      $('#composer-input').value = 'queue draft'
      await actions.queueComposerMessage()
      check(state.messageQueues[key]?.[0]?.text === 'queue draft' && !$('#composer-input').value, `${backend}: queue action`)
      actions.openQueuedMessageEditor('queued')
      check($('#edit-queued-message-dialog').open, `${backend}: queue editor opens`)
      $('#edit-queued-message-text').value = 'edited queue'
      await actions.saveEditedQueuedMessage({ preventDefault: noop })
      check(state.messageQueues[key][0].text === 'edited queue' && !$('#edit-queued-message-dialog').open, `${backend}: queue editor saves`)
      await actions.deleteQueuedMessage('queued')
      check(!state.messageQueues[key], `${backend}: queue delete`)
      render(); actions.quickSendContinueMessage()
      check(submissions === 1 && $('#composer-input').value.trim(), `${backend}: quick Continue submits`)
      $('#composer-input').value = ''; state.continueBehavior = 'ollamaDraft'; state.translation = { ollamaModel: 'fixture' }
      render(); await actions.draftContinueMessage()
      check($('#composer-input').value === 'reviewed continuation' && submissions === 1, `${backend}: draft Continue never submits`)
      state.model.activeTurnId = 'stop-fixture'
      await actions.interruptTurn()
      check(state.pausedMessageQueues.has(key) && actionRequests.includes('turn/interrupt'), `${backend}: Stop pauses queue and interrupts`)
      state.routerMode = true
      state.model.activeTurnId = 'routing'
      render()
      check($('#send-message').disabled, `${backend}: active Router must not offer unsupported Steer`)
      state.model.activeTurnId = null
      render()
      check(!$('#send-message').disabled, `${backend}: idle Router must accept another target request`)
      reports.push(`${backend}: idle, draft, send, running, completion, disconnect, rejection, queue edit/delete, Continue, Stop, Router busy/idle PASS`)
      fixture.remove()
    }
    return reports
  })
}

export async function checkCommentMarkerScope(page) {
  const result = await page.evaluate(async () => {
    const { createReviewNotesController, createReviewNotesState } = await import('/review-notes-controller.mjs')
    const root = document.createElement('section')
    root.id = 'document-marker-acceptance'
    root.style.cssText = 'width:650px;padding:24px;background:var(--panel);color:var(--text)'
    root.innerHTML = '<div id="transcript"><div id="fixture-item"><div class="markdown-body">sample text</div></div></div><div id="artifact-content" class="markdown-body">sample text</div>'
    document.body.append(root)
    const state = { ...createReviewNotesState(), selectedId: 'fixture', artifactView:'preview', artifact:{path:'/fixture.md',root:'/',kind:'text',hash:'v1',content:'sample text'} }
    const view = { selectedStateKey: () => 'fixture', renderedItem: () => root.querySelector('#fixture-item') }
    root.insertAdjacentHTML('beforeend','<div id="selection-popover" class="hidden"><button id="selection-favorite">Favorite</button></div><dialog id="annotation-dialog"><h2 id="annotation-dialog-title"></h2><div id="annotation-source-hint"></div><blockquote id="annotation-quote"></blockquote><textarea id="annotation-comment"></textarea><div id="annotation-error"></div><button id="save-annotation">Save</button></dialog>')
    const controller = createReviewNotesController({ state, view, commentSources: {describe:()=>'/fixture.md'}, gatewayFetch() {}, randomId: () => 'x', persistAnnotationState() {} })
    controller.bind()
    state.annotationDrafts.fixture = [{ id: 'doc', excerpt: 'sample', source: { provider: 'document', anchor: { filePath: '/fixture.md' } } }]
    controller.renderCommentMarkers()
    if (root.querySelector('.chat-comment-anchor')) throw Error('Document comment unexpectedly used a chat marker')
    state.annotationDrafts.fixture.push({ id: 'chat', excerpt: 'sample', source: { provider: 'chat', anchor: { turnId: 't', itemId: 'i' } } })
    controller.renderCommentMarkers()
    if (!root.querySelector('#transcript .chat-comment-anchor')) throw Error('Chat marker missing')
    controller.renderDocumentCommentMarkers()
    const content = root.querySelector('#artifact-content')
    if (content.querySelector('.chat-comment-anchor')?.textContent !== 'sample') throw Error('Document marker missing')
    if (state.artifact.content !== 'sample text') throw Error('Marking changed the file')
    content.innerHTML = 'sample <strong>text</strong>'
    state.annotationDrafts.fixture[0].excerpt = 'sample text'
    controller.renderDocumentCommentMarkers()
    if (content.querySelectorAll('.chat-comment-anchor').length !== 2) throw Error('Cross-inline selection was not marked')
    if (!content.querySelector('strong')) throw Error('Inline formatting lost')
    const before = content.getBoundingClientRect().height
    state.annotationDrafts.fixture = []
    controller.renderDocumentCommentMarkers()
    if (content.querySelector('.chat-comment-anchor, [data-comment-ids], [role="button"]')) throw Error('Clear left active markers')
    if (content.textContent !== 'sample text' || content.getBoundingClientRect().height !== before) throw Error('Clear changed text or layout')
    state.annotationDrafts.fixture = [{id:'doc',excerpt:'sample',source:{provider:'document',anchor:{filePath:'/fixture.md'}}}]
    content.innerHTML = 'sample text'
    controller.renderDocumentCommentMarkers()
    if (!content.querySelector('.chat-comment-anchor')) throw Error('Reopen lost document markers')
    state.artifact.path = '/other.md'
    controller.renderDocumentCommentMarkers()
    if (content.querySelector('.chat-comment-anchor')) throw Error('Marker leaked to another file')
    state.artifact.path = '/fixture.md'
    state.artifactView = 'edit'
    controller.renderDocumentCommentMarkers()
    if (content.querySelector('.chat-comment-anchor')) throw Error('Preview marker touched editor')
    state.artifactView = 'preview'
    controller.renderDocumentCommentMarkers()
    content.querySelector('.chat-comment-anchor').click()
    if (!root.querySelector('#annotation-dialog').open || state.editingAnnotationId !== 'doc') throw Error('Click did not open the matching comment')
    await new Promise(resolve=>setTimeout(resolve,40))
    root.querySelector('#annotation-dialog').close()
    content.innerHTML = 'repeat repeat'
    const range = document.createRange()
    range.setStart(content.firstChild,7); range.setEnd(content.firstChild,13)
    window.getSelection().removeAllRanges(); window.getSelection().addRange(range)
    content.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}))
    if (state.pendingSelection.source.anchor.previewStartOffset !== 7 || state.pendingSelection.source.anchor.previewEndOffset !== 13) throw Error('Selection did not capture rendered offsets')
    state.annotationDrafts.fixture = [{id:'repeat',excerpt:'repeat',source:state.pendingSelection.source}]
    window.getSelection().removeAllRanges()
    root.querySelector('#selection-popover').classList.add('hidden')
    controller.renderDocumentCommentMarkers()
    if (content.querySelector('.chat-comment-anchor')?.previousSibling?.textContent !== 'repeat ') throw Error('Repeated selection marked the wrong occurrence')
    return 'PASS: chat and Markdown markers isolated; inline markup, clear without layout change, reopen, other-file and editor isolation'
  })
  if (process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR,{recursive:true})
    await page.locator('#document-marker-acceptance').screenshot({path:join(process.env.STUDIO_SCREENSHOT_DIR,'markdown-comment-markers.png')})
  }
  await page.locator('#document-marker-acceptance').evaluate(node=>node.remove())
  return result
}
