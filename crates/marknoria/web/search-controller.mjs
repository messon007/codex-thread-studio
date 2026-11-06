// One bounded, revision-owned search session per reader pane. DOM/navigation
// stay in adapters; hosts activate verified occurrence ordinals, never coordinates.
const WINDOW = 100, RENDERER_LIMIT = 500
const error = (code, message) => Object.assign(Error(message), { code })
const cancelled = (message = 'Search was superseded or cancelled') => Object.assign(error('CANCELLED', message), { name: 'AbortError' })
export function searchController(options) {
  let identity = null, query = '', document = null, revision = null, mode = null
  let matches = [], offset = 0, total = 0, available = 0, exact = true
  let current = null, active = null, loading = false, failure = null
  let rendererMatches = null, serial = 0, request = null, disposed = false
  const owner = () => options.owner()
  const sameOwner = value => !disposed && value.documentId === owner().documentId && value.revision === owner().revision
  function capabilities() {
    const value = owner()
    return { version: 1, queryBytes: 256, windowLimit: WINDOW, pagination: value.renderer ? 'capped' : 'source', resultLimit: value.renderer ? RENDERER_LIMIT : null, activation: !value.editing, views: value.source ? ['preview', 'source'] : ['preview'] }
  }
  function state() {
    return { version: 1, documentId: document, revision, searchId: identity, query, offset, current, total, totalExact: exact, available, loading, error: failure, active }
  }
  const changed = () => options.changed?.(state())
  function cancel() {
    serial++; request?.abort(); request = null
    options.cancel?.()
  }
  function clear() {
    cancel()
    identity = null; query = ''; document = revision = mode = null
    matches = []; rendererMatches = null; offset = total = available = 0
    current = active = null; exact = true; loading = false; failure = null
    options.clear?.(); changed()
    return response()
  }
  function response() {
    return { ...state(), mode, limit: WINDOW, complete: exact && offset + matches.length >= total, matches }
  }
  function operation(signal) {
    signal?.throwIfAborted()
    cancel()
    const ticket = serial, expected = owner(), controller = new AbortController()
    request = controller
    const abort = () => { controller.abort(); if (ticket === serial) options.cancel?.() }
    signal?.addEventListener('abort', abort, { once: true })
    const check = () => {
      controller.signal.throwIfAborted()
      if (ticket !== serial) throw cancelled()
      if (!sameOwner(expected)) throw error('REVISION', 'Search document or revision changed')
    }
    return { ticket, expected, signal: controller.signal, check,
      finish() { signal?.removeEventListener('abort', abort); if (request === controller) request = null } }
  }
  function describe(items, start) {
    return items.map((match, position) => ({ ...options.describe(match), index: start + position }))
  }
  async function windowAt(start, op) {
    const capped = Boolean(op.expected.renderer)
    let response
    if (capped) {
      if (!rendererMatches) {
        response = await options.load(query, 0, op.expected, op.signal)
        op.check()
        if (!Array.isArray(response.matches) || response.matches.length > RENDERER_LIMIT) throw error('BUDGET', 'Renderer search exceeds its declared result budget')
        mode = response.mode ?? 'renderer'
        rendererMatches = describe(response.matches, 0)
        exact = response.complete === true
        total = available = rendererMatches.length
      }
      if (start > available) throw error('RANGE', 'Search offset exceeds the available renderer results')
      return rendererMatches.slice(start, start + WINDOW)
    }
    response = await options.load(query, start, op.expected, op.signal)
    op.check()
    if (response.revision !== op.expected.revision) throw error('REVISION', 'Search revision changed')
    if (!Array.isArray(response.matches) || response.matches.length > WINDOW || !Number.isSafeInteger(response.total) || response.total < 0 || response.offset !== start) throw error('BUDGET', 'Invalid bounded search response')
    mode = response.mode ?? 'case-sensitive-source-lines'
    total = available = response.total; exact = true
    if (start > total) throw error('RANGE', 'Search offset exceeds the match count')
    return describe(response.matches, start)
  }
  async function search(params, signal) {
    if (disposed) throw error('DISPOSED', 'Search controller disposed')
    signal?.throwIfAborted()
    const text = params.query, start = params.offset === undefined ? 0 : params.offset
    if (typeof text !== 'string' || new TextEncoder().encode(text).length > 256 || !Number.isSafeInteger(start) || start < 0) throw error('PARAMS', 'Invalid bounded search query or offset')
    if (!text) { if (start !== 0) throw error('PARAMS', 'An empty search cannot have an offset'); signal?.throwIfAborted(); return clear() }
    const expected = owner()
    if (!expected.documentId || !expected.revision) throw error('BUSY', 'No ready document to search')
    if (query !== text || document !== expected.documentId || revision !== expected.revision) {
      clear(); identity = crypto.randomUUID(); query = text; document = expected.documentId; revision = expected.revision
    }
    const op = operation(signal)
    loading = true; failure = null; changed()
    try {
      const items = await windowAt(start, op); op.check()
      matches = items; offset = start; loading = false; options.paint?.(query, active)
      return response()
    } catch (cause) {
      if (op.ticket === serial) failure = cause.name === 'AbortError' ? null : { code: cause.code || 'SEARCH', message: String(cause.message).slice(0, 800) }
      throw op.signal.aborted || op.ticket !== serial || cause.name === 'AbortError' ? cancelled(cause.message) : cause
    } finally {
      op.finish(); if (op.ticket === serial) { loading = false; changed() }
    }
  }
  async function activate(params, signal) {
    if (disposed) throw error('DISPOSED', 'Search controller disposed')
    if (!identity || params.searchId !== identity) throw error('SEARCH_ID', 'Unknown or expired search identity')
    if (!Number.isSafeInteger(params.index) || params.index < 0 || params.index >= available) throw error('RANGE', 'Search occurrence is outside the available results')
    if (owner().editing) throw error('CAPABILITY', 'Search activation requires Preview or Source')
    if (document !== owner().documentId || revision !== owner().revision) throw error('REVISION', 'Search document or revision changed')
    const op = operation(signal), target = params.index
    loading = true; failure = null; current = active = null; options.paint?.(query, null); changed()
    try {
      if (target < offset || target >= offset + matches.length) {
        const start = Math.floor(target / WINDOW) * WINDOW
        const items = await windowAt(start, op); op.check()
        matches = items; offset = start
      }
      const match = matches[target - offset]
      if (!match || match.index !== target) throw error('RANGE', 'Search occurrence is unavailable')
      await options.activate(match, { query, signal: op.signal }); op.check()
      active = match; current = target; loading = false; options.paint?.(query, active)
      return state()
    } catch (cause) {
      if (op.ticket === serial) {
        options.paint?.(query, null)
        failure = cause.name === 'AbortError' ? null : { code: cause.code || 'SEARCH', message: String(cause.message).slice(0, 800) }
      }
      throw op.signal.aborted || op.ticket !== serial || cause.name === 'AbortError' ? cancelled(cause.message) : cause
    } finally {
      op.finish(); if (op.ticket === serial) { loading = false; changed() }
    }
  }
  return { search, activate, clear, state, response, capabilities,
    repaint() { if (!loading && !disposed) options.paint?.(query, active) },
    suspend() { cancel(); loading = false; options.paint?.('', null); changed() },
    dispose() { clear(); disposed = true },
  }
}
