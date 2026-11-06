import { installReaderBridge } from './reader-bridge.mjs'
import { queryOperation } from './query-operation.mjs'

export function publicReader({ origins, state, index, api, renderer = () => null, search, setView, navigate, navigateFragment, contents, selection, setCommentMarks, notesChanged, save, settings, dispose }) {
  let imageRequests = 0
  const queryId = id => {
    if (typeof id !== 'string' || !/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(id)) throw Error('Invalid query identity')
    return id
  }
  const current = params => {
    const value = state()
    if (value.state !== 'ready') throw Object.assign(Error('Reader is preparing its document renderer'), { code: 'BUSY' })
    if (!value.documentId || params.documentId !== value.documentId || params.revision !== value.revision) {
      throw Object.assign(Error('Reader document or revision changed'), { code: 'REVISION' })
    }
    return value
  }
  const checked = execute => (params, signal) => { current(params); return execute(params, signal) }
  function searchParams(params, fields) {
    const allowed = new Set(['documentId', 'revision', ...fields])
    if (Object.keys(params).some(key => !allowed.has(key))) throw Object.assign(Error('Unsupported search parameter'), { code: 'PARAMS' })
  }
  return installReaderBridge({ origins, state, onDispose: dispose, commands: {
    setView: checked(async params => { if (!['preview', 'source', 'edit'].includes(params.view)) throw Error('Invalid reader view'); await setView(params.view); return state() }),
    navigate: checked(async (params, signal) => { signal.throwIfAborted(); await navigate(params.location, { signal }); signal.throwIfAborted(); current(params); return state() }),
    navigateFragment: checked(async params => {
      if (typeof params.fragment !== 'string' || !params.fragment.length || new TextEncoder().encode(params.fragment).length>8192 || typeof navigateFragment !== 'function') throw Error('Invalid or unsupported document fragment')
      await navigateFragment(params.fragment);return state()
    }),
    contents: checked(async params => { if (typeof params.open !== 'boolean' || typeof contents !== 'function') throw Error('Invalid Contents command'); await contents(params.open); return state() }),
    save: checked(async () => { await save(); return state() }),
    selection: checked((params, signal) => {
      if (params.textOnly !== undefined && typeof params.textOnly !== 'boolean') throw TypeError('Invalid text selection filter')
      return selection(signal, { textOnly: params.textOnly === true })
    }),
    setCommentMarks: checked(params => { if (typeof setCommentMarks !== 'function') throw Error('Host comment marks are unavailable'); return setCommentMarks(params.marks) }),
    referenceInfo: checked((params,signal) => {
      if (!Number.isSafeInteger(params.id) || params.id < 0 || params.id >= (index().references?.length ?? 0)) throw Error('Invalid reference identity')
      return api(`/api/reference?id=${params.id}&revision=${encodeURIComponent(params.revision)}`,signal)
    }),
    referenceImageInfo: checked((params,signal) => {
      if (!Number.isSafeInteger(params.id) || index().references?.[params.id]?.kind !== 'image') throw Error('Invalid image reference')
      return api(`/api/reference-image?id=${params.id}&revision=${encodeURIComponent(params.revision)}`,signal)
    }),
    referenceImageFrame: checked(async (params,signal) => {
      if (!Number.isSafeInteger(params.id) || index().references?.[params.id]?.kind !== 'image' || !Number.isSafeInteger(params.frame) || params.frame < 0 || params.frame >= 128 || !/^[a-f0-9]{64}$/i.test(params.imageRevision || '')) throw Error('Invalid referenced image frame')
      if (imageRequests >= 2) throw Object.assign(Error('Image frame budget exhausted'),{code:'BUSY'})
      imageRequests++
      try {
        const blob = await api(`/api/reference-image?id=${params.id}&revision=${encodeURIComponent(params.revision)}&frame=${params.frame}&imageRevision=${encodeURIComponent(params.imageRevision)}`,signal,{imageRevision:params.revision})
        return { revision:params.revision,id:params.id,frame:params.frame,imageRevision:params.imageRevision,blob }
      } finally { imageRequests-- }
    }),
    assetInfo: checked((params, signal) => {
      if (!Number.isSafeInteger(params.id) || params.id < 0) throw Error('Invalid asset identity')
      return api(`/api/asset-info?id=${params.id}&revision=${encodeURIComponent(params.revision)}`, signal)
    }),
    assetFrame: checked(async (params, signal) => {
      if (!Number.isSafeInteger(params.id) || params.id < 0 ||
          !Number.isSafeInteger(params.frame) || params.frame < 0 ||
          !index().assets[params.id]?.frames[params.frame]) throw Error('Invalid image frame')
      if (imageRequests >= 2) throw Object.assign(Error('Image frame budget exhausted'), { code: 'BUSY' })
      imageRequests++
      try {
        const blob = await api(`/api/asset?id=${params.id}&frame=${params.frame}`, signal, { imageRevision: params.revision })
        return { revision: params.revision, id: params.id, frame: params.frame, blob }
      } finally { imageRequests-- }
    }),
    projectionWindow: checked((params, signal) => {
      if (renderer() || !Number.isSafeInteger(params.start) || params.start < 0 || !Number.isSafeInteger(params.count) || params.count < 1 || params.count > 5) throw Error('Invalid bounded projection window')
      return api(`/api/projection?revision=${encodeURIComponent(params.revision)}&start=${params.start}&count=${params.count}`, signal)
    }),
    sourceWindow: checked((params, signal) => {
      if (!state().capabilities.source || params.line != null && (!Number.isSafeInteger(params.line) || params.line < 1) || params.offset != null && (!Number.isSafeInteger(params.offset) || params.offset < 0) || params.line != null && params.offset != null || params.previous != null && typeof params.previous !== 'boolean' || params.end != null && (!Number.isSafeInteger(params.end) || params.offset == null || params.line != null || params.previous === true || params.end < params.offset || params.end-params.offset > 128*1024)) throw Error('Invalid bounded source window')
      const query = new URLSearchParams()
      if (params.line != null) query.set('line',params.line)
      if (params.offset != null) query.set('offset',params.offset)
      if (params.end != null) query.set('end',params.end)
      query.set('previous',params.previous === true)
      return api(`/api/source?${query}`,signal)
    }),
    sourceSelection: checked((params, signal) => {
      if (!state().capabilities.source || ![params.start,params.end,params.fromUtf16,params.toUtf16].every(value=>Number.isSafeInteger(value)&&value>=0) || params.end<params.start || params.toUtf16<params.fromUtf16) throw Error('Invalid source selection coordinates')
      return api('/api/selection/source',signal,{method:'POST',body:JSON.stringify({revision:params.revision,start:params.start,end:params.end,fromUtf16:params.fromUtf16,toUtf16:params.toUtf16})})
    }),
    outline: checked((params, signal) => {
      if (params.resolve != null && typeof params.resolve !== 'boolean') throw Error('Invalid outline resolution option')
      if (renderer()) return renderer().outline({ offset: params.offset ?? 0, resolve: params.resolve ?? true, signal })
      const offset = params.offset ?? 0, headings = index().toc
      if (!Number.isSafeInteger(offset) || offset < 0 || offset > headings.length) throw Error('Invalid outline offset')
      return { revision: state().revision, offset, total: headings.length, items: headings.slice(offset, offset + 256) }
    }),
    navigateOutline: checked(async (params, signal) => {
      if (!Number.isSafeInteger(params.id) || params.id < 0) throw Error('Invalid outline identity')
      if (renderer()) await renderer().navigateOutline(params.id, { signal })
      else {
        const heading = index().toc.find(item => item.id === params.id)
        if (!heading) throw Error('Unknown outline identity')
        await navigate({ kind: 'line', line: heading.line })
      }
      return state()
    }),
    search: checked((params, signal) => {
      searchParams(params, ['query', 'offset', 'matchCase'])
      if (!search) throw Object.assign(Error('Search is unavailable'), { code: 'CAPABILITY' })
      return search.search(params, signal)
    }),
    searchActivate: checked(async (params, signal) => {
      searchParams(params, ['searchId', 'index'])
      if (!search) throw Object.assign(Error('Search activation is unavailable'), { code: 'CAPABILITY' })
      await search.activate(params, signal)
      current(params); return state()
    }),
    searchClear: checked((params, signal) => {
      searchParams(params, [])
      signal.throwIfAborted()
      if (!search) throw Object.assign(Error('Search is unavailable'), { code: 'CAPABILITY' })
      search.clear(); return state()
    }),
    sql: checked((params, signal) => {
      if (!state().capabilities.queries) throw Error('Queries were disabled by the host')
      if (!Number.isSafeInteger(params.table) || params.table < 0 || typeof params.sql !== 'string' || params.sql.length > 16384) throw Error('Invalid SQL request')
      return queryOperation(api, params.revision, signal, operationId => api('/api/table-query', signal, { method: 'POST', body: JSON.stringify({ table: params.table, sql: params.sql, revision: params.revision, operationId }) }))
    }),
    querySnapshot: checked((params, signal) => {
      if (!state().capabilities.querySnapshots || params.location?.kind !== 'queryCell' || typeof params.quote !== 'string' || new TextEncoder().encode(params.quote).length > 32768) throw Error('Invalid or disabled SQL snapshot request')
      return api('/api/query-snapshot', signal, {method:'POST',body:JSON.stringify({revision:params.revision,location:params.location,quote:params.quote,quoteTruncated:params.quoteTruncated === true})})
    }),
    tableChart: checked((params, signal) => {
      if (!state().capabilities.charts) throw Error('Charts were disabled or no table is available')
      if (!Number.isSafeInteger(params.y) || params.y < 0 || params.x != null && (!Number.isSafeInteger(params.x) || params.x < 0)) throw Error('Invalid chart axes')
      return queryOperation(api, params.revision, signal, operationId => api('/api/table-chart', signal, { method: 'POST', body: JSON.stringify({ source: params.source, x: params.x ?? null, y: params.y, revision: params.revision, operationId }) }))
    }),
    jq: checked((params, signal) => {
      if (!state().capabilities.queries || typeof params.filter !== 'string' || params.filter.length > 16384) throw Error('Invalid or disabled jq request')
      return queryOperation(api, params.revision, signal, operationId => api('/api/jq-query', signal, { method: 'POST', body: JSON.stringify({ filter: params.filter, slurp: params.slurp === true, revision: params.revision, operationId }) }))
    }),
    epubProgress: checked(async(params,signal)=>{
      const active=renderer()
      if(state().format!=='epub'||!active)throw Error('EPUB progress requires an EPUB document')
      if(params.operation==='build')await active.buildProgressIndex({signal})
      else if(params.operation==='cancel')active.cancelProgressIndex()
      else if(params.operation==='seek')await active.seekProgress(params.percentage)
      else throw Error('Invalid EPUB progress operation')
      return state()
    }),
    notes: checked((params, signal) => {
      if (!state().capabilities.notes) throw Error('Note storage was not granted by the host')
      if (!params.request || !['list','get','marks','add','update','delete','clear','copy'].includes(params.request.operation) || new TextEncoder().encode(JSON.stringify(params.request)).length>131072) throw Error('Invalid bounded note request')
      return api('/api/notes',signal,{method:'POST',body:JSON.stringify(params.request)}).then(value => { if (['add','update','delete','clear'].includes(params.request.operation)) notesChanged?.(); return value })
    }),
    tableInfo: checked(params => {
      if (!Number.isSafeInteger(params.table) || params.table < 0 || !index().tables[params.table]) throw Error('Invalid table identity')
      const table = index().tables[params.table]
      return { revision: state().revision, table: params.table, columns: table.columns, rows: table.rows, label: table.label || 'Table' }
    }),
    tableWindow: checked((params, signal) => {
      if (!Number.isSafeInteger(params.table) || params.table < 0 || !Number.isSafeInteger(params.row) || params.row < 0 || !Array.isArray(params.columns) || !params.columns.length || params.columns.length > 512 || new Set(params.columns).size !== params.columns.length || params.columns.some(column => !Number.isSafeInteger(column) || column < 0)) throw Error('Invalid table window')
      return api(`/api/table?id=${params.table}&row=${params.row}&columns=${params.columns.join(',')}`, signal)
    }),
    sqlSchema: checked((params, signal) => {
      if (!Number.isSafeInteger(params.table) || params.table < 0) throw Error('Invalid table identity')
      return api(`/api/table-query/schema?table=${params.table}`, signal)
    }),
    sqlPage: checked((params, signal) => {
      if (!Number.isSafeInteger(params.row) || params.row < 0) throw Error('Invalid result row')
      return api(`/api/table-query?id=${queryId(params.id)}&row=${params.row}`, signal)
    }),
    sqlClose: checked((params, signal) => api('/api/table-query/close', signal, { method: 'POST', body: JSON.stringify({ id: queryId(params.id) }) })),
    jqPage: checked((params, signal) => {
      if (!Number.isSafeInteger(params.page) || params.page < 0) throw Error('Invalid result page')
      return api('/api/jq-query', signal, { method: 'POST', body: JSON.stringify({ id: queryId(params.id), page: params.page, revision: params.revision }) })
    }),
    jqClose: checked((params, signal) => api('/api/jq-query', signal, { method: 'POST', body: JSON.stringify({ id: queryId(params.id), close: true, revision: params.revision }) })),
    settings: checked(async params => { await settings(params); return state() }),
  } })
}
