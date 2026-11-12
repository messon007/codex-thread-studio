// Studio shell adapter over the fixed public MarkNoria parent protocol.
// It never reads the child DOM, diagnostic globals or implementation controls.
export function createLargeMarkdownHost(frame, onState, onSelection = () => {}, options = {}) {
  let client, disposed = false, snapshot = { state: 'indexing' }, request, generation = 0
  let heartbeat = null, signature = '', initialized = false
  let appliedPresentation = {}, presentationQueue = Promise.resolve()
  const presentationDelta = (next, previous) => Object.fromEntries(Object.entries(next).flatMap(([key, value]) => {
    if (value === undefined) return []
    if (key === 'epub' && value && typeof value === 'object') {
      const changed = presentationDelta(value, previous[key] || {})
      return Object.keys(changed).length ? [[key, changed]] : []
    }
    return JSON.stringify(value) === JSON.stringify(previous[key]) ? [] : [[key, value]]
  }))
  const rememberPresentation = patch => {
    const epub = patch.epub ? { ...appliedPresentation.epub, ...patch.epub } : appliedPresentation.epub
    appliedPresentation = { ...appliedPresentation, ...patch, ...(epub ? { epub } : {}) }
  }
  let heartbeatRequest = null, heartbeatTimeout = null, terminalError = null, heartbeatFailure = ''
  const lifetime = new AbortController()
  const translate = options.translate || (value => value)
  const stopHeartbeat = () => {
    clearInterval(heartbeat); clearTimeout(heartbeatTimeout)
    heartbeatRequest?.abort()
    lifetime.abort()
  }
  const publish = state => {
    if (disposed || !state) return
    // Once the host lease is gone, late child events must not restore readiness.
    if (terminalError) state = { ...snapshot, state: 'error', busy: false, error: terminalError.message, editorError: null }
    if (snapshot.documentId && (snapshot.documentId !== state.documentId || snapshot.revision !== state.revision || snapshot.view !== state.view)) {
      onSelection(null)
      request?.abort(); ++generation
    }
    snapshot = state
    const visible = {
      initialized, ready: initialized && state.state === 'ready', readingReady: state.timings?.firstUsefulContentMs != null,
      editable: Boolean(state.capabilities?.edit), view: state.view, dirty: Boolean(state.dirty),
      busy: Boolean(state.busy && state.capabilities?.edit), outlineOpen: Boolean(state.outline?.open), hasOutline: Boolean(state.outline?.available),
      lineCount: state.document?.sourceLines, size: state.document?.fileBytes, name: state.name,
      documentId: state.documentId, revision: state.revision, format: state.format,
      status: state.editorError?.message || state.error || '', error: state.error,
      position: state.position, search: state.search, capabilities: state.capabilities, formatState: state.formatState,
    }
    const next = JSON.stringify(visible)
    if (next !== signature) { signature = next; onState(visible) }
  }
  const capture = selection => {
    if (disposed || terminalError) return
    if (!selection) { onSelection(null); return }
    if (!selection.quote || selection.documentId !== snapshot.documentId || selection.revision !== snapshot.revision) return
    const local = selection.viewportRect, outer = frame.getBoundingClientRect()
    const line = selection.location?.kind === 'line' ? selection.location.line : null
    onSelection({ ...selection, startLine: line, endLine: line,
      rect: local ? { left: outer.left + local.x, top: outer.top + local.y,
        right: outer.left + local.x + local.width, bottom: outer.top + local.y + local.height,
        width: local.width, height: local.height } : null })
  }
  const ready = (async () => {
    const factory = options.createClient || (await import(new URL('./reader-assets/host-client.mjs',import.meta.url).href)).createReaderClient
    if (disposed) return
    client = factory(frame,{onEvent(type,payload){
      if (disposed || terminalError) return
      if (type==='ready'||type==='state') publish(payload)
      else if (type==='selection') capture(payload)
      else if (type==='commentRequested' && payload?.documentId===snapshot.documentId && payload.revision===snapshot.revision && (payload.id===null || typeof payload.id==='string' && payload.id.length<=256)) options.onCommentRequested?.(payload)
      else if (type==='openRequested') options.onOpenRequested?.()
      else if (type==='linkRequested' && payload?.documentId && payload.documentId===snapshot.documentId && payload.revision===snapshot.revision && typeof payload.target==='string' && payload.target.length<=8192 && new TextEncoder().encode(payload.target).length<=8192) {
        void Promise.resolve().then(()=>{if(!disposed)return options.onLinkRequested?.(payload)}).catch(error=>{if(!disposed)options.onError?.(error)})
      }
      else if (type==='error') options.onError?.(payload)
    }})
    publish(await client.ready())
    if (disposed) { await client.dispose({force:true}); return }
    // Equal typography must not reset the shared reader's layout measurements.
    // Compare initialization with public state, then retain only acknowledged settings.
    appliedPresentation = structuredClone(snapshot.presentation || {})
    const initialPresentation = structuredClone({...options.presentation,toolbar:'host',notes:'host',selectionMenu:'host'})
    const initialPatch = presentationDelta(initialPresentation, appliedPresentation)
    if (Object.keys(initialPatch).length) publish(await client.request('settings',initialPatch))
    rememberPresentation(initialPresentation)
    if (disposed) return
    const initialView = options.initialView
    if (initialView === 'source' && snapshot.capabilities?.source || initialView === 'edit' && snapshot.capabilities?.edit) publish(await client.request('setView',{view:initialView}))
    if (disposed) return
    if (options.initialReference) {
      validateReference(options.initialReference)
      await client.request('navigate',{location:options.initialReference.location})
    } else if (options.initialLocation) await client.request('navigate',{location:options.initialLocation})
    if (options.initialFragment) await client.request('navigateFragment',{fragment:options.initialFragment})
    if (disposed) return
    initialized = true
    publish(snapshot)
    const keepAlive = options.heartbeat || (async signal => {
      const url = new URL(frame.src)
      const response = await fetch(new URL('./api/capabilities', url), {
        headers: { 'x-reader-token': url.hash.slice(1), 'x-reader-document': snapshot.documentId },
        signal, cache: 'no-store',
      })
      // Only the status is needed; release the response body on every path.
      void response.body?.cancel().catch(() => {})
      if (!response.ok) throw Object.assign(Error('Reader keepalive failed (' + response.status + ')'), { status: response.status })
    })
    const pulse = async () => {
      if (disposed || terminalError || heartbeatRequest) return
      const operation = heartbeatRequest = new AbortController()
      heartbeatTimeout = setTimeout(() => operation.abort(), 10000)
      try {
        await keepAlive(operation.signal)
        if (!disposed) heartbeatFailure = ''
      } catch (error) {
        if (disposed || terminalError) return
        if ([401, 403, 404, 410].includes(error?.status)) {
          terminalError = new Error(translate(error.status === 410 || error.status === 404
            ? 'Document closed or expired. Refresh to reopen it. Unsaved changes will require confirmation.'
            : 'Reader authorization expired. Reconnect to Studio, then refresh the document.'))
          stopHeartbeat()
          request?.abort(); ++generation
          onSelection(null)
          publish(snapshot)
          options.onError?.(terminalError)
        } else {
          // A transient outage remains retryable, with one request at a time
          // and one report per failure until connectivity recovers.
          const message = operation.signal.aborted ? 'Reader keepalive timed out' : String(error?.message || error)
          if (message !== heartbeatFailure) { heartbeatFailure = message; options.onError?.(new Error(message)) }
        }
      } finally {
        clearTimeout(heartbeatTimeout)
        if (heartbeatRequest === operation) heartbeatRequest = null
      }
    }
    heartbeat = setInterval(() => { void pulse() }, 30000)
    const ownerDocument = frame.ownerDocument
    const resume = () => { if (ownerDocument?.visibilityState !== 'hidden') void pulse() }
    ownerDocument?.addEventListener('visibilitychange', resume, { signal: lifetime.signal })
    ownerDocument?.defaultView?.addEventListener('pageshow', resume, { signal: lifetime.signal })
    ownerDocument?.defaultView?.addEventListener('online', resume, { signal: lifetime.signal })
    return snapshot
  })()
  void ready.catch(error=>{if(!disposed){publish({...snapshot,state:'error',error:String(error.message||error)});options.onError?.(error)}})
  async function command(method,params,signal) {
    await ready
    if (disposed || !client) throw new DOMException('Reader host disposed','AbortError')
    if (terminalError) throw terminalError
    return client.request(method,params,{signal})
  }
  async function searchCommand(method, params, owner) {
    request?.abort()
    const operation = request = new AbortController(), own = ++generation
    const check = () => {
      operation.signal.throwIfAborted()
      if (disposed || own !== generation) throw new DOMException('Search superseded', 'AbortError')
    }
    await ready
    check()
    if (snapshot.capabilities?.search?.version !== 1) throw Error('This reader does not support public search activation')
    const expected = owner || { documentId: snapshot.documentId, revision: snapshot.revision }
    const value = await command(method, { ...params, ...expected }, operation.signal)
    check()
    if (method === 'search') {
      const { matches, ...search } = value
      publish({ ...snapshot, search })
    } else publish(value)
    return value
  }
  function validateReference(reference) {
      if(reference.revision!==snapshot.revision)throw Error('The document has changed since this reference was captured')
      if(reference.location?.kind==='source'&&!snapshot.capabilities?.sourceRangeNavigation)throw Error('This reader package does not support exact source-range navigation')
      if(reference.precision==='draft')throw Error('An unsaved draft reference cannot be restored after reopening')
      if(reference.location?.kind==='queryCell')throw Error('This SQL result reference is transient; run the original query again')
  }
  return {
    ready,
    get busy(){return Boolean(snapshot.busy && snapshot.capabilities?.edit)}, get dirty(){return Boolean(snapshot.dirty)},
    async setView(view){publish(await command('setView',{view}))},
    async save(){publish(await command('save',{}));return true},
    async setOutline(open){publish(await command('contents',{open}))},
    settings(settings) {
      const desired = structuredClone(settings)
      // Serialize per iframe; a failed command must remain retryable. Comparing
      // after the previous acknowledgement also coalesces repeated render updates.
      const operation = presentationQueue.then(async () => {
        await ready
        if (disposed) throw new DOMException('Reader host disposed', 'AbortError')
        const patch = presentationDelta(desired, appliedPresentation)
        if (!Object.keys(patch).length) return
        const documentId = snapshot.documentId
        const next = await command('settings', patch, lifetime.signal)
        if (disposed || snapshot.documentId !== documentId) throw new DOMException('Reader owner changed', 'AbortError')
        rememberPresentation(patch)
        publish(next)
      })
      presentationQueue = operation.catch(() => {})
      return operation
    },
    request(method, params, { signal } = {}) { return command(method, params, signal) },
    querySnapshot(selection, { signal } = {}) {
      return command('querySnapshot',{documentId:selection.documentId,revision:selection.revision,location:selection.location,quote:selection.quote,quoteTruncated:selection.quoteTruncated === true},signal)
    },
    notes(request, { documentId, revision, signal }) { return command('notes',{documentId,revision,request},signal) },
    navigateLine(line){return command('navigate',{location:{kind:'line',line}})},
    navigateLocation(location){return command('navigate',{location})},
    navigateFragment(fragment){return command('navigateFragment',{fragment})},
    async navigateReference(reference){
      await ready
      validateReference(reference)
      return command('navigate',{location:reference.location})
    },
    search(query, offset = 0, matchCase = snapshot.capabilities?.search?.defaultMatchCase) { return searchCommand('search', { query, offset, matchCase }) },
    activateSearch(search, index) {
      return searchCommand('searchActivate', { searchId: search.searchId, index },
        { documentId: search.documentId, revision: search.revision })
    },
    clearSearch() { return searchCommand('searchClear', {}) },
    leave(){return this.dispose()},
    dispose(){
      if(disposed)return
      onSelection(null)
      disposed=true;++generation;request?.abort();stopHeartbeat()
      // The surrounding Studio discard prompt is authoritative. Backend close
      // separately awaits service cleanup; no credentials enter the child.
      const closing=client?.dispose({force:true})
      void closing?.catch(()=>{})
      client?.disconnect?.()
      return closing
    },
  }
}

// Studio owns selection actions; the SDK only supplies typed, revision-bound
// references. Cancelling a selection also cancels its pending SQL provenance.
export function createReaderSelectionHost(options = {}) {
  let disposed = false, owner = null, host = null, pending = null
  const clear = () => { pending?.abort(); pending = null; options.onSelection?.(null) }
  return {
    update(next, transport) {
      if (disposed) return
      if (owner && (next.documentId !== owner.documentId || next.revision !== owner.revision || next.view !== owner.view)) clear()
      owner = next; host = transport
    },
    async capture(selection) {
      if (disposed) return
      clear()
      if (!selection?.quote || !owner?.ready || selection.documentId !== owner.documentId || selection.revision !== owner.revision) return
      const operation = pending = new AbortController()
      try {
        const reference = await readerCommentReference(host, selection, operation.signal)
        operation.signal.throwIfAborted()
        if (disposed || pending !== operation) return
        options.onSelection?.(reference)
      } catch (error) {
        if (!operation.signal.aborted && !disposed) options.onError?.(error)
      }
    },
    dispose() { if (disposed) return; disposed = true; clear(); owner = host = null },
  }
}

export async function readerCommentReference(host, selection, signal) {
  signal.throwIfAborted()
  // Persisted Studio excerpts have both UTF-16 and UTF-8 budgets. Copy keeps
  // the original bounded selection; comments and translations use the excerpt.
  let quote = selection.quote
  if (quote.length > 16000) {
    quote = quote.slice(0,16000)
    const last = quote.charCodeAt(quote.length-1)
    if (last >= 0xd800 && last <= 0xdbff) quote = quote.slice(0,-1)
  }
  const encoded = new TextEncoder().encode(quote)
  if (encoded.length > 16384) {
    let end = 16384
    while (end > 0 && (encoded[end] & 0xc0) === 0x80) end--
    quote = new TextDecoder().decode(encoded.subarray(0,end))
  }
  const excerpt = { ...selection, quote, copyText: selection.quote, ...(quote === selection.quote ? {} : {quoteTruncated:true}) }
  const reference = excerpt.location?.kind === 'queryCell'
    ? {...excerpt, querySnapshot: await host.querySnapshot(excerpt, {signal})} : excerpt
  signal.throwIfAborted()
  return reference
}

// Host toolbar entry; all query UI, budgets and cancellation stay in the SDK.
export function createReaderJqHost(root, toolbar, options = {}) {
  let disposed = false, generation = 0, key = '', button = null, dialog = null, controls = null
  const lifetime = new AbortController()
  const load = options.load || (() => import(new URL('./reader-assets/jq-query.mjs', import.meta.url).href))
  const close = () => { const owned = dialog; dialog = null; void Promise.resolve(owned?.dispose()).catch(() => {}) }
  const label = () => { if (button) button.title = options.translate?.('Query JSON with jq') || 'Query JSON with jq' }
  options.translate?.subscribe?.(label, { signal: lifetime.signal })
  return {
    update(owner, host) {
      if (disposed) return
      const enabled = owner.initialized && owner.ready && owner.capabilities?.queries && /\.(json|jsonl|ndjson)$/iu.test(owner.name || '')
      const next = enabled ? owner.documentId + ':' + owner.revision : ''
      if (next === key) return
      key = next
      const own = ++generation
      close(); controls?.abort(); controls = null; button?.remove(); button = null
      if (!next) return
      controls = new AbortController()
      const control = button = root.ownerDocument.createElement('button')
      control.type = 'button'; control.textContent = 'jq'; control.setAttribute('aria-label', 'jq'); label()
      control.addEventListener('click', async () => {
        control.disabled = true
        try {
          const { mountJqQuery, readerJqProvider } = await load()
          if (disposed || own !== generation) return
          close()
          dialog = mountJqQuery(root.ownerDocument.body, {
            provider: readerJqProvider(host, owner), revision: owner.revision,
            translate: options.translate, presentationRoot: root, autoRun: Number.isSafeInteger(owner.size) && owner.size <= 1024 * 1024,
          })
        } catch (error) { if (!disposed && own === generation) options.onError?.(error) }
        finally { if (!disposed && own === generation) control.disabled = false }
      }, { signal: controls.signal })
      toolbar.append(control)
    },
    dispose() { if (disposed) return; disposed = true; generation++; lifetime.abort(); controls?.abort(); close(); button?.remove(); button = null },
  }
}
