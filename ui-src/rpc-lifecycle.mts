interface PendingRequest { resolve: (value: unknown) => void; reject: (error: unknown) => void; timer: ReturnType<typeof setTimeout>; method: string }
const record = (value: unknown): Record<string, unknown> => value !== null && typeof value === 'object' ? value as Record<string, unknown> : {}

export class PendingRpcRequests {
  constructor(readonly pending: Map<string, PendingRequest>) {}
  request(id: string | number, method: string, send: () => void, timeoutMs: number, timeoutMessage: string): Promise<unknown> {
    const key = String(id)
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(key); reject(new Error(timeoutMessage)) }, timeoutMs)
      this.pending.set(key, { resolve, reject, timer, method })
      try { send() } catch (error) { clearTimeout(timer); this.pending.delete(key); reject(error) }
    })
  }
  settle(value: unknown): boolean {
    const message = record(value)
    const key = String(message.id)
    const pending = this.pending.get(key)
    if (!pending) return false
    this.pending.delete(key)
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(String(record(message.error).message || JSON.stringify(message.error))))
    else pending.resolve(message.result)
    return true
  }
  rejectAll(error: unknown): void {
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(error) }
    this.pending.clear()
  }
}

export interface RpcSocket {
  onmessage: ((event: { data: string }) => void) | null
  onerror: (() => void) | null
  onclose: (() => void) | null
  send(value: string): void
  close(): void
}

export function requestSocketRpc(options: {
  socket: RpcSocket; id: number; method: string; params: unknown; timeoutMs: number
  timeoutMessage: string; unavailableMessage: string; connectMessage: string; closeMessage: string
  responseError: (error: unknown) => string
}): Promise<unknown> {
  const { socket, id, method, params } = options
  return new Promise((resolve, reject) => {
    let requested = false
    let settled = false
    const finish = (error: Error | null, value?: unknown) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      socket.onclose = null
      socket.close()
      if (error) reject(error)
      else resolve(value)
    }
    const timer = setTimeout(() => finish(new Error(options.timeoutMessage)), options.timeoutMs)
    socket.onmessage = event => {
      if (settled) return
      let message: Record<string, unknown>
      try { message = record(JSON.parse(event.data)) } catch { return }
      const status = record(message.params)
      if (message.method === 'studio/appServer/status' && status.state === 'error') {
        finish(new Error(String(status.message || options.unavailableMessage)))
      } else if (message.method === 'studio/appServer/status' && status.state === 'ready' && !requested) {
        requested = true
        try { socket.send(JSON.stringify({ id, method, params })) } catch (error) { finish(error instanceof Error ? error : new Error(String(error))) }
      } else if (message.id === id) {
        if (message.error) finish(new Error(options.responseError(message.error)))
        else finish(null, message.result)
      }
    }
    socket.onerror = () => finish(new Error(options.connectMessage))
    socket.onclose = () => finish(new Error(options.closeMessage))
  })
}

export interface SelectedConnectionState {
  socket: RpcSocket | null
  socketGeneration: number
  ready: boolean
  reconnectTimer: ReturnType<typeof setTimeout> | null
}
export function connectSelectedSocket(state: SelectedConnectionState, effects: {
  cleanup: () => void
  starting: () => void
  open: () => RpcSocket
  message: (data: string) => void
  error: () => void
  closed: () => void
  reconnect: () => void
}): void {
  if (state.reconnectTimer != null) clearTimeout(state.reconnectTimer)
  effects.cleanup()
  state.ready = false
  const generation = ++state.socketGeneration
  effects.starting()
  const socket = effects.open()
  state.socket = socket
  socket.onmessage = event => { if (generation === state.socketGeneration) effects.message(event.data) }
  socket.onerror = () => { if (generation === state.socketGeneration) effects.error() }
  socket.onclose = () => {
    if (generation !== state.socketGeneration) return
    state.ready = false
    effects.closed()
    state.reconnectTimer = setTimeout(effects.reconnect, 1800)
  }
}
