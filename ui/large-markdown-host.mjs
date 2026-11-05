// Studio shell adapter over the fixed public MarkNoria parent protocol.
// It never reads the child DOM, diagnostic globals or implementation controls.
export function createLargeMarkdownHost(frame, onState, onSelection = () => {}, options = {}) {
  let client, disposed = false, snapshot = { state: 'indexing' }, request, generation = 0
  let heartbeat = null, signature = '', initialized = false
  const publish = state => {
    if (disposed || !state) return
    snapshot = state
    const visible = {
      initialized, ready: initialized && state.state === 'ready', readingReady: state.timings?.firstUsefulContentMs != null,
      editable: Boolean(state.capabilities?.edit), view: state.view, dirty: Boolean(state.dirty),
      busy: Boolean(state.busy && state.capabilities?.edit), outlineOpen: Boolean(state.outline?.open), hasOutline: Boolean(state.outline?.available),
      lineCount: state.document?.sourceLines, size: state.document?.fileBytes, name: state.name,
      documentId: state.documentId, revision: state.revision, format: state.format,
      status: state.editorError?.message || state.error || '', error: state.error,
      position: state.position, capabilities: state.capabilities, formatState: state.formatState,
    }
    const next = JSON.stringify(visible)
    if (next !== signature) { signature = next; onState(visible) }
  }
  const capture = selection => {
    if (disposed || !selection?.quote || selection.documentId !== snapshot.documentId || selection.revision !== snapshot.revision) return
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
      if (disposed) return
      if (type==='ready'||type==='state') publish(payload)
      else if (type==='selection') capture(payload)
      else if (type==='openRequested') options.onOpenRequested?.()
      else if (type==='linkRequested' && payload?.documentId && payload.documentId===snapshot.documentId && payload.revision===snapshot.revision && typeof payload.target==='string' && payload.target.length<=8192 && new TextEncoder().encode(payload.target).length<=8192) {
        void Promise.resolve().then(()=>{if(!disposed)return options.onLinkRequested?.(payload)}).catch(error=>{if(!disposed)options.onError?.(error)})
      }
      else if (type==='error') options.onError?.(payload)
    }})
    publish(await client.ready())
    if (disposed) { await client.dispose({force:true}); return }
    publish(await client.request('settings',{toolbar:'host',...(snapshot.capabilities?.presentation?.notesOwner ? {notes:'host'} : {}),...options.presentation}))
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
    const keepAlive = options.heartbeat || (async()=>{
      const url=new URL(frame.src)
      const response=await fetch(new URL('./api/capabilities',url),{headers:{'x-reader-token':url.hash.slice(1),'x-reader-document':snapshot.documentId}})
      if(!response.ok)throw Error('Reader keepalive failed ('+response.status+')')
    })
    heartbeat=setInterval(()=>{if(!disposed)Promise.resolve(keepAlive()).catch(error=>options.onError?.(error))},30000)
    return snapshot
  })()
  void ready.catch(error=>{if(!disposed){publish({...snapshot,state:'error',error:String(error.message||error)});options.onError?.(error)}})
  async function command(method,params,signal) {
    await ready
    if (disposed || !client) throw new DOMException('Reader host disposed','AbortError')
    return client.request(method,params,{signal})
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
    async settings(settings){publish(await command('settings',settings))},
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
    async search(query){
      request?.abort();const own=++generation
      if(!query.trim()||disposed)return []
      const operation=request=new AbortController()
      const value=await command('search',{query},operation.signal)
      return own===generation&&!disposed?value.matches:[]
    },
    navigateMatch(match){
      const location=snapshot.view==='preview'&&match.location?match.location:{kind:'line',line:match.line}
      return command('navigate',{location})
    },
    leave(){return this.dispose()},
    dispose(){
      if(disposed)return
      disposed=true;++generation;request?.abort();clearInterval(heartbeat)
      // The surrounding Studio discard prompt is authoritative. Backend close
      // separately awaits service cleanup; no credentials enter the child.
      const closing=client?.dispose({force:true})
      void closing?.catch(()=>{})
      client?.disconnect?.()
      return closing
    },
  }
}

// Ordinary host Notes UI over the same explicit Rust store as the child reader.
// Each binding captures its document/revision and transport; no active-pane lookup.
export function createReaderNotesHost(root, toolbar, options = {}) {
  let disposed = false, generation = 0, key = '', binding = null, selected = null, enabled = false
  const load = options.load || (() => import(new URL('./reader-assets/notes.mjs', import.meta.url).href))
  return {
    async update(owner, host) {
      if (disposed) return
      enabled = Boolean(owner.capabilities?.notes && owner.capabilities?.presentation?.notesOwner)
      const next = enabled && owner.initialized && owner.ready ? owner.documentId + ':' + owner.revision : ''
      if (key === next) return
      key = next
      const own = ++generation
      binding?.dispose(); binding = null; selected = null
      if (!next) return
      const { mountNotes } = await load()
      if (disposed || own !== generation) return
      const identity = { documentId: owner.documentId, revision: owner.revision, name: owner.name || 'Document' }
      binding = mountNotes({
        owner: identity, root, toolbar, overlayRoot: root.ownerDocument.body,
        selection: async () => selected,
        storage: (request, { signal }) => host.notes(request, { ...identity, signal }),
        canSave: () => !host.dirty,
        queryNotes: Boolean(owner.capabilities.queryNotes),
        translate: options.translate,
        actions: options.onAction ? [{ id: 'ask-ai', label: 'Ask AI', run: async (selection, { signal }) => {
          signal.throwIfAborted()
          if (disposed || own !== generation) throw new DOMException('Notes owner changed','AbortError')
          if (selection.documentId !== identity.documentId || selection.revision !== identity.revision) throw Error('Notes owner changed')
          // Studio drafts have a 16 KiB UTF-8 excerpt budget. Capture a verified
          // prefix before asking for provenance; never split a Unicode scalar.
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
          const excerpt = quote === selection.quote ? selection : {...selection,quote,quoteTruncated:true}
          const reference = excerpt.location?.kind === 'queryCell'
            ? {...excerpt, querySnapshot: await host.querySnapshot(excerpt, {signal})} : excerpt
          signal.throwIfAborted()
          if (disposed || own !== generation) throw new DOMException('Notes owner changed','AbortError')
          selected = null
          await binding?.selectionChanged()
          signal.throwIfAborted()
          return options.onAction(reference, { signal })
        } }] : [],
      })
      if (selected) await binding.selectionChanged()
    },
    capture(value) {
      if (!enabled || disposed) return false
      const bounds = value.rect
      selected = { ...value, viewportRect: bounds ? { x: bounds.left, y: bounds.top, width: bounds.width, height: bounds.height } : value.viewportRect }
      void binding?.selectionChanged().catch(error => options.onError?.(error))
      return true
    },
    dispose() {
      if (disposed) return
      disposed = true; ++generation; binding?.dispose(); binding = null; selected = null
    },
  }
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
