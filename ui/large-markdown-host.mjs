// Studio shell adapter over the fixed public MarkNoria parent protocol.
// It never reads the child DOM, diagnostic globals or implementation controls.
export function createLargeMarkdownHost(frame, onState, onSelection = () => {}, options = {}) {
  let client, disposed = false, snapshot = { state: 'indexing' }, request, generation = 0
  let heartbeat = null, signature = ''
  const publish = state => {
    if (disposed || !state) return
    snapshot = state
    const visible = {
      ready: state.state === 'ready', readingReady: state.timings?.firstUsefulContentMs != null,
      editable: Boolean(state.capabilities?.edit), view: state.view, dirty: Boolean(state.dirty),
      busy: Boolean(state.busy), outlineOpen: Boolean(state.outline?.open), hasOutline: Boolean(state.outline?.available),
      lineCount: state.document?.sourceLines, size: state.document?.fileBytes,
      documentId: state.documentId, revision: state.revision, format: state.format,
      status: state.editorError?.message || state.error || '', error: state.error,
      position: state.position, capabilities: state.capabilities,
    }
    const next = JSON.stringify(visible)
    if (next !== signature) { signature = next; onState(visible) }
  }
  const capture = selection => {
    if (disposed || !selection?.quote) return
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
      if (type==='ready'||type==='state') publish(payload)
      else if (type==='selection') capture(payload)
      else if (type==='error') options.onError?.(payload)
    }})
    publish(await client.ready())
    if (disposed) { await client.dispose({force:true}); return }
    publish(await client.request('settings',{toolbar:'host',...options.presentation}))
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
  return {
    ready,
    get busy(){return Boolean(snapshot.busy)}, get dirty(){return Boolean(snapshot.dirty)},
    async setView(view){publish(await command('setView',{view}))},
    async save(){publish(await command('save',{}));return true},
    async setOutline(open){publish(await command('contents',{open}))},
    async settings(settings){publish(await command('settings',settings))},
    navigateLine(line){return command('navigate',{location:{kind:'line',line}})},
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
      void closing?.catch(error=>options.onError?.(error))
      return closing
    },
  }
}
