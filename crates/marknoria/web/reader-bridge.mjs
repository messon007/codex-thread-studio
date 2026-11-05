// Versioned, file-scoped parent protocol. No private DOM or diagnostic globals.
export const READER_PROTOCOL = 'marknoria.reader'
export const READER_BRIDGE_VERSION = 1
const uuid = value => typeof value === 'string' && /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(value)
export function installReaderBridge({ origins, commands, state, onDispose = () => {} }) {
  const lifetime = new AbortController(), running = new Map()
  const allowed = new Set([location.origin, ...origins])
  let peer = null, lastState = '', disposed = false, disposal
  const send = data => { if (peer) parent.postMessage({ protocol: READER_PROTOCOL, version: READER_BRIDGE_VERSION, ...data }, peer) }
  function publish(type, payload) {
    if (disposed || !peer) return
    if (type === 'state') { const signature = JSON.stringify(payload); if (signature === lastState) return; lastState = signature }
    send({ kind: 'event', type, payload })
  }
  function dispose() {
    if (disposed) return disposal
    publish('disposed', { ...state(), state: 'disposed' })
    disposed = true; lifetime.abort()
    for (const operation of running.values()) operation.abort()
    running.clear(); peer = null
    disposal = Promise.resolve().then(onDispose)
    return disposal
  }
  addEventListener('message', async event => {
    if (disposed || event.source !== parent || !allowed.has(event.origin)) return
    const message = event.data
    if (!message || message.protocol !== READER_PROTOCOL || !uuid(message.requestId)) return
    const reply = data => parent.postMessage({ protocol: READER_PROTOCOL, version: READER_BRIDGE_VERSION, kind: 'response', requestId: message.requestId, ...data }, event.origin)
    if (message.version !== READER_BRIDGE_VERSION) { reply({ error: { code: 'VERSION', message: 'Unsupported reader bridge version' } }); return }
    if (message.kind === 'cancel') { running.get(message.requestId)?.abort(); return }
    if (message.kind !== 'request') return
    if (running.has(message.requestId)) return
    if (running.size >= 8) { reply({ error: { code: 'BUSY', message: 'Reader command budget exhausted' } }); return }
    const controller = new AbortController(); running.set(message.requestId, controller)
    try {
      if (typeof message.method !== 'string' || message.method.length > 64) throw Error('Invalid reader command')
      peer = event.origin
      let result
      if (message.method === 'hello' || message.method === 'state') result = state()
      else if (message.method === 'dispose') {
        result = state()
        if (result.dirty && message.params?.force !== true) throw Object.assign(Error('Unsaved edits remain; save or explicitly discard before closing'), { code: 'DIRTY' })
        result = { ...result, state: 'disposed' }; await dispose(); reply({ result }); return
      }
      else {
        const execute = Object.hasOwn(commands, message.method) && commands[message.method]
        if (typeof execute !== 'function') { reply({ error: { code: 'UNSUPPORTED', message: 'Unsupported reader command' } }); return }
        result = await execute(message.params ?? {}, controller.signal)
      }
      controller.signal.throwIfAborted()
      reply({ result })
      if (message.method === 'hello' && state().state === 'ready') publish('ready', state())
      publish('state', state())
    } catch (error) {
      reply({ error: { code: controller.signal.aborted ? 'CANCELLED' : error.code || 'COMMAND', message: String(error.message).slice(0, 1024) } })
    } finally { running.delete(message.requestId) }
  }, { signal: lifetime.signal })
  return { publish, dispose, get connected() { return peer != null && !disposed }, get active() { return running.size } }
}
