import { codexLifecycleStreamMessage } from './codex-lifecycle-diagnostics.mjs'
import type { RpcSocket } from './rpc-lifecycle.mjs'

export interface LifecycleEventSource {
  onopen: (() => void) | null
  onmessage: ((event: { data: string }) => void) | null
  onerror: (() => void) | null
}
export interface EventStreamState {
  stream: LifecycleEventSource | null
  ready: boolean
  missedBarrier: boolean
  waiters: Set<(ready: boolean) => void>
  openCount: number
}

export function waitForEventStream(state: EventStreamState, timeoutMs = 1_500): Promise<boolean> {
  if (state.ready) return Promise.resolve(true)
  return new Promise(resolve => {
    let settled = false
    const finish = (ready: boolean) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      state.waiters.delete(finish)
      resolve(ready)
    }
    const timer = setTimeout(() => finish(false), timeoutMs)
    state.waiters.add(finish)
  })
}

export function connectEventStream(state: EventStreamState, effects: {
  open: () => LifecycleEventSource
  connected: (needsReconciliation: boolean) => void
  message: (payload: unknown) => void
  invalid: (error: unknown, data: string) => void
  disconnected: () => void
}): LifecycleEventSource {
  if (state.stream) return state.stream
  const events = effects.open()
  state.stream = events
  events.onopen = () => {
    const needsReconciliation = state.openCount > 0 || state.missedBarrier
    state.openCount += 1
    state.ready = true
    state.missedBarrier = false
    for (const waiter of [...state.waiters]) waiter(true)
    effects.connected(needsReconciliation)
  }
  events.onmessage = event => {
    try { effects.message(JSON.parse(event.data)) }
    catch (error) { effects.invalid(error, event.data) }
  }
  events.onerror = () => {
    state.ready = false
    effects.disconnected()
  }
  return events
}

export interface LifecycleConnectionState {
  socket: RpcSocket | null
  reconnectTimer: ReturnType<typeof setTimeout> | null
  ready: boolean
  generation: number
  openCount: number
}
export interface LifecycleConnectionEffects {
  open: () => RpcSocket
  invalid: (error: unknown, data: string) => void
  report: (details: { state: string; backendCount?: number }) => void
  reconcile: () => void
  supports: (backend: string) => boolean
  status: (backend: string, message: Record<string, unknown>) => void
  lag: (backend: string, message: Record<string, unknown>) => void
  notification: (backend: string, message: Record<string, unknown>) => void
}

export function connectLifecycleStream(state: LifecycleConnectionState, effects: LifecycleConnectionEffects): Promise<boolean> {
  if (state.reconnectTimer !== null) clearTimeout(state.reconnectTimer)
  state.reconnectTimer = null
  if (state.socket) {
    state.socket.onclose = null
    state.socket.close()
  }
  state.ready = false
  state.generation += 1
  const generation = state.generation
  const socket = effects.open()
  state.socket = socket

  return new Promise<boolean>((resolve) => {
    let settled = false
    const settle = (ready: boolean) => {
      if (settled) return
      settled = true
      clearTimeout(barrierTimer)
      resolve(ready)
    }
    // This is only an initialization ordering barrier. The connection stays
    // alive and can become authoritative after a slow local WebSocket upgrade.
    const barrierTimer = setTimeout(() => settle(false), 750)

    socket.onmessage = (event) => {
      if (generation !== state.generation) return
      let envelope
      try { envelope = codexLifecycleStreamMessage(JSON.parse(event.data)) }
      catch (error) {
        effects.invalid(error, event.data)
        return
      }
      if (!envelope) return
      if (envelope.type === 'ready') {
        const reconnected = state.openCount > 0
        state.openCount += 1
        state.ready = true
        settle(true)
        effects.report({
          state: reconnected ? 'reconnected' : 'connected',
          backendCount: envelope.backends.length,
        })
        if (reconnected) effects.reconcile()
        return
      }
      if (!effects.supports(envelope.backend)) return
      if (envelope.message.method === 'studio/appServer/status') {
        effects.status(envelope.backend, envelope.message)
      } else if (envelope.message.method === 'studio/appServer/lagged') {
        effects.lag(envelope.backend, envelope.message)
      } else {
        effects.notification(envelope.backend, envelope.message)
      }
    }
    socket.onerror = () => {
      if (generation !== state.generation) return
      state.ready = false
    }
    socket.onclose = () => {
      if (generation !== state.generation) return
      state.ready = false
      state.socket = null
      settle(false)
      effects.report({ state: 'disconnected' })
      state.reconnectTimer = setTimeout(() => {
        if (generation !== state.generation) return
        connectLifecycleStream(state, effects)
      }, 1_800)
    }
  })
}
