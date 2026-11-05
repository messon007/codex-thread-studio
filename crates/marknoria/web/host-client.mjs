// Host-side consumer. It imports no reader implementation and inspects no DOM.
import { READER_PROTOCOL, READER_BRIDGE_VERSION } from './reader-bridge.mjs'
export function createReaderClient(frame, { origin = new URL(frame.src, location.href).origin, timeout = 30000, readyTimeout = 75000, onEvent = () => {} } = {}) {
  if (!/^https?:\/\//.test(origin) || new URL(origin).origin !== origin) throw Error('An exact reader service origin is required')
  const lifetime = new AbortController(), pending = new Map()
  let disposed = false, current = null, closing = null
  addEventListener('message', event => {
    if (event.source !== frame.contentWindow || event.origin !== origin) return
    const data = event.data
    if (data?.protocol !== READER_PROTOCOL || data.version !== READER_BRIDGE_VERSION) return
    if (data.kind === 'event') { if (data.type === 'ready' || data.type === 'state') current = data.payload; if (data.type === 'disposed') { current = data.payload; if (![...pending.values()].some(entry => entry.method === 'dispose')) cleanup() }; onEvent(data.type, data.payload); return }
    if (data.kind !== 'response') return
    const entry = pending.get(data.requestId)
    if (!entry) return
    if (data.result?.bridgeVersion === READER_BRIDGE_VERSION) current = data.result
    entry.finish(data.error ? Object.assign(new Error(data.error.message), { code: data.error.code }) : null, data.result)
    if (entry.method === 'dispose' && (!data.error || current?.state === 'disposed')) cleanup()
  }, { signal: lifetime.signal })
  function sendRequest(method, params = {}, { signal } = {}, internal = false) {
    if (disposed || closing && !internal) return Promise.reject(Error('Reader client disposed or closing'))
    if (pending.size >= 32) return Promise.reject(Error('Host request budget exhausted'))
    if (signal?.aborted) return Promise.reject(signal.reason)
    if (!['hello', 'state', 'dispose'].includes(method)) params = { documentId: current?.documentId, revision: current?.revision, ...params }
    const requestId = crypto.randomUUID()
    return new Promise((resolve, reject) => {
      let retry
      const finish = (error, value) => {
        clearTimeout(timer); clearInterval(retry); signal?.removeEventListener('abort', abort)
        pending.delete(requestId); error ? reject(error) : resolve(value)
      }
      const abort = () => {
        frame.contentWindow?.postMessage({ protocol: READER_PROTOCOL, version: READER_BRIDGE_VERSION, kind: 'cancel', requestId }, origin)
        finish(signal?.reason || new DOMException('Cancelled', 'AbortError'))
      }
      const timer = setTimeout(() => { frame.contentWindow?.postMessage({ protocol: READER_PROTOCOL, version: READER_BRIDGE_VERSION, kind: 'cancel', requestId }, origin); finish(Object.assign(new Error('Reader command timed out'), { code: 'TIMEOUT' })); }, Math.max(100, Math.min(timeout, 120000)))
      pending.set(requestId, { finish, method })
      signal?.addEventListener('abort', abort, { once: true })
      const send = () => frame.contentWindow?.postMessage({ protocol: READER_PROTOCOL, version: READER_BRIDGE_VERSION, kind: 'request', requestId, method, params }, origin)
      send()
      // A hello sent before the child installs its listener must not be lost.
      if (method === 'hello') retry = setInterval(send, 250)
    })
  }
  const request = (method, params, options) => sendRequest(method, params, options)
  function cleanup() {
    disposed = true; lifetime.abort()
    for (const entry of [...pending.values()]) entry.finish(Error('Reader client disposed'))
  }
  async function dispose({ force = false } = {}) {
    if (disposed) return
    if (closing) return closing
    closing = (async () => {
      if (!current) await sendRequest('hello', {}, {}, true)
      await sendRequest('dispose', { force }, {}, true)
      cleanup()
    })()
    try { await closing } finally { if (force) cleanup(); closing = null }
  }
  async function ready() {
    const deadline = Date.now() + Math.max(100, Math.min(readyTimeout, 120000))
    while (!disposed && Date.now() < deadline) {
      const result = await request('hello')
      if (result.state === 'ready') return result
      if (result.state === 'error') throw Error(result.error || 'Document indexing failed')
      await new Promise(resolve => setTimeout(resolve, 100))
    }
    throw Error('Reader did not become ready')
  }
  // The owner may remove a frame while independently closing its service.
  // Disconnect only releases this client's listeners, retries and pending calls;
  // it does not acknowledge reader/service disposal or grant new authority.
  return { request, ready, dispose, disconnect: cleanup, get active() { return pending.size } }
}
